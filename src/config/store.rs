use std::cell::Cell;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::error::{io_with_context, BzrError, Result};

use super::Config;

impl Config {
    /// Resolve the path to the config file.
    #[cfg(test)]
    pub fn path() -> Result<PathBuf> {
        Self::path_at(None)
    }

    /// Resolve the config path for one invocation.
    ///
    /// Precedence: the explicit invocation path > `BZR_CONFIG` > the default
    /// config directory.
    pub fn path_at(path_override: Option<&Path>) -> Result<PathBuf> {
        if let Some(path) = path_override {
            return Ok(path.to_path_buf());
        }
        Self::path_from_environment()
    }

    fn path_from_environment() -> Result<PathBuf> {
        if let Some(env_path) = std::env::var_os("BZR_CONFIG") {
            if !env_path.is_empty() {
                return Ok(PathBuf::from(env_path));
            }
        }
        let config_dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(dirs::config_dir)
            .ok_or_else(|| BzrError::config("cannot determine config directory"))?;
        Ok(config_dir.join("bzr").join("config.toml"))
    }

    fn ensure_config_dir_at(path_override: Option<&Path>) -> Result<PathBuf> {
        let path = Self::path_at(path_override)?;
        let parent = path
            .parent()
            .ok_or_else(|| BzrError::config("config path has no parent directory"))?
            .to_path_buf();
        let parent_exists = parent.exists();
        fs::create_dir_all(&parent).map_err(|e| {
            io_with_context(
                format!("create config directory '{}'", parent.display()),
                &e,
            )
        })?;
        if !parent_exists {
            set_private_directory_permissions(&parent)?;
        }
        Ok(parent)
    }

    /// Read and parse the config from disk WITHOUT validating it or warning on
    /// permissions. Maps a missing file to `Config::default()`. Used by
    /// update-locked writes and by `load_at`.
    ///
    /// `pub(crate)` so `config remove-server`/`rename-server` can take an
    /// advisory snapshot (existence, default pointer, keyring ref) even when
    /// some unrelated server has invalid fields.
    pub(crate) fn read_unvalidated_at(path_override: Option<&Path>) -> Result<Config> {
        let path = Self::path_at(path_override)?;
        match fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).map_err(|e| {
                BzrError::config(format!("parse config file '{}': {e}", path.display()))
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(io_with_context(
                format!("read config file '{}'", path.display()),
                &e,
            )),
        }
    }

    #[cfg(test)]
    pub fn load() -> Result<Config> {
        Self::load_at(None)
    }

    pub fn load_at(path_override: Option<&Path>) -> Result<Config> {
        // Warn on insecure permissions only on an explicit load (preserves today's
        // behavior); `update_locked_at`'s internal reload uses `read_unvalidated`,
        // so it warns once (from `write_to_disk`) rather than twice.
        let path = Self::path_at(path_override)?;
        if path.exists() {
            Self::warn_on_insecure_permissions(&path);
        }
        let config = Self::read_unvalidated_at(path_override)?;
        config.validate()?;
        Ok(config)
    }

    #[cfg(test)]
    pub(super) fn save(&self) -> Result<()> {
        self.validate()?;
        self.write_to_disk()
    }

    /// Apply `mutator` to the config under an exclusive advisory lock, with a
    /// reload from disk *inside* the lock so concurrent processes editing
    /// disjoint fields do not clobber each other.
    ///
    /// The lock (`config.lock`, sibling of `config.toml`) is held only across
    /// the in-memory mutation and the atomic write — never across interactive
    /// I/O. The closure must therefore be self-contained and non-interactive:
    /// run any prompt, keyring, or network step *before* calling this. Because
    /// the config is reloaded from disk first, the closure must not rely on
    /// unpersisted in-memory state, and should upsert (create-if-absent) any
    /// server it means to create.
    ///
    /// Returns the freshly-applied config so callers can use the post-write
    /// state without a second read.
    ///
    /// Non-reentrant: a `mutator` that itself calls `update_locked` returns an
    /// error rather than self-deadlocking.
    #[cfg(test)]
    pub fn update_locked(mutator: impl FnOnce(&mut Config) -> Result<()>) -> Result<Config> {
        Self::update_locked_at(None, mutator)
    }

    pub fn update_locked_at(
        path_override: Option<&Path>,
        mutator: impl FnOnce(&mut Config) -> Result<()>,
    ) -> Result<Config> {
        Self::update_locked_inner(path_override, true, mutator)
    }

    /// Test-only no-argument wrapper around
    /// [`Self::update_locked_without_validation_at`].
    #[cfg(test)]
    pub fn update_locked_without_validation(
        mutator: impl FnOnce(&mut Config) -> Result<()>,
    ) -> Result<Config> {
        Self::update_locked_without_validation_at(None, mutator)
    }

    pub fn update_locked_without_validation_at(
        path_override: Option<&Path>,
        mutator: impl FnOnce(&mut Config) -> Result<()>,
    ) -> Result<Config> {
        Self::update_locked_inner(path_override, false, mutator)
    }

    fn update_locked_inner(
        path_override: Option<&Path>,
        validate: bool,
        mutator: impl FnOnce(&mut Config) -> Result<()>,
    ) -> Result<Config> {
        if LOCK_HELD.with(Cell::get) {
            return Err(BzrError::config(
                "internal error: Config::update_locked_at called re-entrantly \
                 (a mutation closure must not write the config itself)",
            ));
        }

        let dir = Self::ensure_config_dir_at(path_override)?;
        let lock_path = dir.join("config.lock");
        let file = open_lock_file(&lock_path)?;
        acquire_exclusive_lock(&file, &lock_path)?;
        LOCK_HELD.with(|held| held.set(true));
        let _guard = LockGuard { file };

        // Reload WITHOUT validation. `Config::load_at` validates unconditionally;
        // validating the reload would make `update_locked_without_validation`
        // fail before a caller can repair or preserve unrelated invalid config.
        // We validate only the post-mutation state when `validate` is true,
        // matching `save()`'s "validate the whole config before writing"
        // semantics.
        let mut config = Self::read_unvalidated_at(path_override)?;
        mutator(&mut config)?;
        if validate {
            config.validate()?;
        }
        config.write_to_disk_at(path_override)?;
        Ok(config)
    }

    /// Persist the config **without** running validation.
    ///
    /// Used only in tests. Applies the same `0o600`/`0o700` hardening as `save`
    /// so a recreated config file is never world-readable.
    #[cfg(test)]
    pub(super) fn save_without_validation(&self) -> Result<()> {
        self.write_to_disk()
    }

    /// Serialize and write the config to its on-disk path atomically:
    /// a uniquely-named sibling temp is written `0o600`, fsync'd (unix),
    /// renamed over the target (atomic replace), and the directory is
    /// fsync'd (unix) so the rename survives a crash. A concurrent reader
    /// therefore always sees either the complete old or complete new file.
    #[cfg(test)]
    fn write_to_disk(&self) -> Result<()> {
        self.write_to_disk_at(None)
    }

    fn write_to_disk_at(&self, path_override: Option<&Path>) -> Result<()> {
        let _dir = Self::ensure_config_dir_at(path_override)?;
        let path = Self::path_at(path_override)?;
        reap_stale_temps(&path);
        let content = toml::to_string_pretty(self).map_err(|e| {
            BzrError::config(format!("serialize config file '{}': {e}", path.display()))
        })?;
        atomic_write(&path, &content)?;
        Self::warn_on_insecure_permissions(&path);
        Ok(())
    }

    fn warn_on_insecure_permissions(path: &std::path::Path) {
        #[cfg(unix)]
        {
            if let Some(parent) = path.parent() {
                warn_if_path_permissions_too_open(parent, 0o077, "config directory");
            }
            if path.exists() {
                warn_if_path_permissions_too_open(path, 0o077, "config file");
            }
        }
    }
}

/// Atomically write `content` to `path`: write a uniquely-named sibling
/// temp file, durably flush it (unix), then rename it over `path`.
/// `rename` replaces the destination atomically on POSIX and on Windows
/// (`MoveFileExW`). The directory is fsync'd on unix so the rename is
/// durable across a crash; on non-unix the concurrent-reader atomicity
/// holds but crash-durability is best-effort.
fn atomic_write(path: &std::path::Path, content: &str) -> Result<()> {
    // Create + write the temp. `create_new` is collision-tolerant: a stale
    // same-pid crash-orphan younger than the reaper's age gate could share
    // the first candidate name, so retry with fresh names before failing.
    let tmp = write_unique_temp(path, content)?;
    // Test-only fault seam: simulate a crash/failure *after* the temp is
    // written but *before* the rename, to deterministically verify that a
    // failed write leaves the previous config intact (CONC-1 atomicity).
    #[cfg(test)]
    if FAIL_AFTER_TEMP.with(std::cell::Cell::get) {
        let _ = fs::remove_file(&tmp);
        return Err(BzrError::config("injected post-temp failure (test)"));
    }
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(io_with_context(
            format!(
                "rename config temp file '{}' to '{}'",
                tmp.display(),
                path.display()
            ),
            &e,
        ));
    }
    fsync_parent_dir(path)
}

#[cfg(test)]
thread_local! {
    /// When set, [`atomic_write`] fails after writing the temp but before the
    /// rename. Lets a test prove a failed write does not destroy the old file.
    static FAIL_AFTER_TEMP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Arm/disarm the [`atomic_write`] post-temp fault seam (test-only).
#[cfg(test)]
pub(super) fn set_fail_after_temp(on: bool) {
    FAIL_AFTER_TEMP.with(|f| f.set(on));
}

thread_local! {
    /// True while this thread holds the config lock inside `update_locked`.
    /// `File::lock` (flock) treats two descriptors in one process as
    /// independent, so a nested `update_locked` would self-deadlock; we
    /// reject re-entry instead.
    static LOCK_HELD: Cell<bool> = const { Cell::new(false) };
}

/// Releases the advisory lock and clears the re-entrancy flag on drop, so an
/// early `?` return or a panic inside the critical section cannot leave the
/// lock held or the flag stuck.
struct LockGuard {
    file: fs::File,
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = self.file.unlock();
        LOCK_HELD.with(|held| held.set(false));
    }
}

/// Open (creating if absent) the `config.lock` file `0600`, ready for an
/// advisory lock. The lock file's *contents* are irrelevant — only the
/// kernel lock on the open description matters — so it is never written to.
#[cfg(unix)]
fn open_lock_file(lock_path: &Path) -> Result<fs::File> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(lock_path)
        .map_err(|e| {
            io_with_context(
                format!("open config lock file '{}'", lock_path.display()),
                &e,
            )
        })
}

#[cfg(not(unix))]
fn open_lock_file(lock_path: &Path) -> Result<fs::File> {
    use std::fs::OpenOptions;

    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|e| {
            io_with_context(
                format!("open config lock file '{}'", lock_path.display()),
                &e,
            )
        })
}

/// Take the exclusive advisory lock, giving the user feedback if another
/// `bzr` process already holds it. A bare blocking `lock()` would hang the
/// CLI with no output under contention; instead we `try_lock` first and only
/// fall back to blocking after printing a one-line notice to stderr, so the
/// wait is visible rather than a silent freeze.
fn acquire_exclusive_lock(file: &fs::File, lock_path: &Path) -> Result<()> {
    let lock_err = |e: std::io::Error| {
        BzrError::config(format!("could not lock {}: {e}", lock_path.display()))
    };
    match file.try_lock() {
        Ok(()) => Ok(()),
        Err(std::fs::TryLockError::WouldBlock) => {
            let _ = writeln!(
                std::io::stderr(),
                "waiting for another bzr process to finish writing the config…"
            );
            file.lock().map_err(lock_err)
        }
        Err(std::fs::TryLockError::Error(e)) => Err(lock_err(e)),
    }
}

/// Max attempts to find an unused temp name. A collision only happens
/// against a stale same-pid orphan younger than the reaper's age gate, so
/// a few retries with fresh counter values is always enough.
const TEMP_CREATE_ATTEMPTS: u32 = 16;

/// The shared filename prefix for this config's sibling temp files:
/// `<config-file-name>.` (e.g. `config.toml.`). Both the temp **creator**
/// ([`candidate_temp_path`]) and the temp **reaper** ([`reap_stale_temps`])
/// derive their match from this one helper, so the two sides cannot drift:
/// every name the creator can produce is one the reaper will recognize.
fn temp_prefix(path: &std::path::Path) -> String {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    format!("{name}.")
}

/// A candidate sibling temp path: `config.toml.<pid>.<counter>.tmp`. The
/// counter is process-global and monotonic, so each call yields a fresh
/// name; combined with the pid this is unique across concurrent writers.
fn candidate_temp_path(path: &std::path::Path) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let name = format!("{}{pid}.{n}.tmp", temp_prefix(path));
    match path.parent() {
        Some(dir) => dir.join(name),
        None => PathBuf::from(name),
    }
}

/// Create a fresh `0600` sibling temp (collision-tolerant) and write
/// `content` to it durably. Returns the temp path on success. On an
/// `AlreadyExists` collision with a stale orphan, retries with a fresh
/// name; on a write/flush failure, removes the temp it created and
/// propagates the error.
fn write_unique_temp(path: &std::path::Path, content: &str) -> Result<PathBuf> {
    for _ in 0..TEMP_CREATE_ATTEMPTS {
        let tmp = candidate_temp_path(path);
        let mut file = match create_new_private(&tmp) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(io_with_context(
                    format!(
                        "create config temp file '{}' for '{}'",
                        tmp.display(),
                        path.display()
                    ),
                    &e,
                ))
            }
        };
        if let Err(e) = file.write_all(content.as_bytes()) {
            let _ = fs::remove_file(&tmp);
            return Err(io_with_context(
                format!("write config temp file '{}'", tmp.display()),
                &e,
            ));
        }
        if let Err(e) = file.sync_all() {
            let _ = fs::remove_file(&tmp);
            return Err(io_with_context(
                format!("fsync config temp file '{}'", tmp.display()),
                &e,
            ));
        }
        return Ok(tmp);
    }
    Err(BzrError::config(format!(
        "could not create a unique config temp file for '{}' after repeated attempts",
        path.display()
    )))
}

#[cfg(unix)]
fn create_new_private(tmp: &std::path::Path) -> std::io::Result<fs::File> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;

    OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(tmp)
}

#[cfg(not(unix))]
fn create_new_private(tmp: &std::path::Path) -> std::io::Result<fs::File> {
    use std::fs::OpenOptions;

    OpenOptions::new().create_new(true).write(true).open(tmp)
}

#[cfg(unix)]
pub(super) fn fsync_parent_dir(path: &std::path::Path) -> Result<()> {
    let Some(dir) = path.parent() else {
        return Ok(());
    };
    let handle = fs::File::open(dir).map_err(|e| {
        BzrError::config(format!(
            "failed to open config parent directory '{}' for fsync: {e}",
            dir.display()
        ))
    })?;
    handle.sync_all().map_err(|e| {
        BzrError::config(format!(
            "failed to fsync config parent directory '{}': {e}",
            dir.display()
        ))
    })
}

#[cfg(not(unix))]
fn fsync_parent_dir(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

/// How old a temp sibling must be before the reaper treats it as a
/// crash orphan. Comfortably longer than any single atomic write, so a
/// *live* temp belonging to a concurrent `bzr` process is never reaped.
const STALE_TEMP_AGE: std::time::Duration = std::time::Duration::from_secs(3600);

/// Remove crash-orphaned `config.toml.*.tmp` siblings **older than
/// [`STALE_TEMP_AGE`]**. A crash between temp-create and rename leaves a
/// unique-named orphan that no graceful cleanup reaps; sweep old ones so
/// they do not accumulate. The age gate is essential: CONC-1 ships before
/// the CONC-2 lock, so two concurrent processes can each have a fresh
/// in-flight temp — reaping unconditionally would delete the other's live
/// temp and make its `rename` fail (lost write). Only temps untouched for
/// an hour — which no live write produces — are removed. The match prefix
/// comes from [`temp_prefix`], the same source [`candidate_temp_path`] uses.
fn reap_stale_temps(path: &std::path::Path) {
    let Some(dir) = path.parent() else {
        return;
    };
    let prefix = temp_prefix(path);
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !(name.starts_with(prefix.as_str()) && name.ends_with(".tmp")) {
            continue;
        }
        let is_old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|mtime| mtime.elapsed().ok())
            .is_some_and(|age| age >= STALE_TEMP_AGE);
        if is_old {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|e| {
        io_with_context(
            format!(
                "set private permissions on config directory '{}'",
                path.display()
            ),
            &e,
        )
    })?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn warn_if_path_permissions_too_open(path: &std::path::Path, mask: u32, kind: &str) {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    let mode = metadata.permissions().mode();
    if mode & mask == 0 {
        return;
    }

    warn_security(&format!(
        "{kind} '{}' has overly broad permissions ({:o}); expected owner-only access. Fix with `chmod {}` '{}'",
        path.display(),
        mode & 0o777,
        if kind == "config directory" { "700" } else { "600" },
        path.display()
    ));
}

fn warn_security(message: &str) {
    let _ = writeln!(std::io::stderr(), "warning: {message}");
}
