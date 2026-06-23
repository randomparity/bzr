use std::cell::Cell;
use std::collections::HashMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

use crate::error::{io_with_context, BzrError, Result};
use crate::types::{ApiMode, AuthMethod, BugTemplate, SavedQuery};

/// Process-wide override for the config file path, set from the global
/// `--config <PATH>` flag. Takes precedence over `BZR_CONFIG` and the default
/// config directory. `RwLock` (not `OnceLock`) so tests can set and clear it.
static CONFIG_PATH_OVERRIDE: RwLock<Option<PathBuf>> = RwLock::new(None);

#[derive(Debug, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct Config {
    pub default_server: Option<String>,
    #[serde(default)]
    pub servers: HashMap<String, ServerConfig>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub templates: HashMap<String, BugTemplate>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub queries: HashMap<String, SavedQuery>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ServerConfig {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_keyring: Option<KeyringRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_method: Option<AuthMethod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_mode: Option<ApiMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_version: Option<String>,
    /// Accept invalid TLS certificates (self-signed, expired, etc.).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tls_insecure: bool,
    /// Path to a PEM-encoded CA certificate for this server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_ca_cert: Option<PathBuf>,
    /// SHA-256 fingerprint of the pinned server certificate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin_sha256: Option<String>,
    /// Issuer DN stored alongside the pin for rotation detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin_issuer: Option<String>,
    /// Base64-encoded raw DER bytes of the issuer SEQUENCE for
    /// tamper-proof issuer comparison.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin_issuer_der: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct KeyringRef {
    /// Keyring service name. Defaults to "bzr" when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    /// Account/username within the service. Defaults to the server name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

impl KeyringRef {
    pub fn service_or_default(&self) -> &str {
        self.service.as_deref().unwrap_or("bzr")
    }

    pub fn account_or_default<'a>(&'a self, server_name: &'a str) -> &'a str {
        self.account.as_deref().unwrap_or(server_name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSourceKind {
    Inline,
    Env,
    Keyring,
}

#[derive(Debug)]
pub enum CredentialSource<'a> {
    Inline(&'a str),
    EnvVar(&'a str),
    Keyring { service: &'a str, account: &'a str },
}

impl CredentialSource<'_> {
    pub fn kind(&self) -> CredentialSourceKind {
        match self {
            CredentialSource::Inline(_) => CredentialSourceKind::Inline,
            CredentialSource::EnvVar(_) => CredentialSourceKind::Env,
            CredentialSource::Keyring { .. } => CredentialSourceKind::Keyring,
        }
    }
}

impl CredentialSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CredentialSourceKind::Inline => "inline",
            CredentialSourceKind::Env => "env",
            CredentialSourceKind::Keyring => "keyring",
        }
    }
}

impl ServerConfig {
    /// Build an ephemeral server backed by an environment-variable credential,
    /// for the inline `--server-url` flow. The result is never written to disk:
    /// auth method and API mode are left unset (detected per-invocation) and TLS
    /// uses the default OS trust store. Construction cannot fail; an unset or
    /// empty env var surfaces later from [`Self::resolve_api_key`].
    #[must_use]
    pub fn from_url_with_env_key(url: String, api_key_env: String, email: Option<String>) -> Self {
        ServerConfig {
            url,
            api_key_env: Some(api_key_env),
            email,
            ..Self::default()
        }
    }

    pub fn tls_config(&self, server_name: &str) -> crate::tls::TlsConfig {
        crate::tls::TlsConfig {
            insecure: self.tls_insecure,
            ca_cert_path: self.tls_ca_cert.clone(),
            pin_sha256: self.tls_pin_sha256.clone(),
            pin_issuer_der: self.tls_pin_issuer_der.clone(),
            server_name: Some(server_name.to_string()),
        }
    }

    pub fn validate(&self, server_name: &str) -> Result<()> {
        self.credential_source()
            .map(|_| ())
            .map_err(|err| BzrError::config(format!("server '{server_name}': {err}")))?;
        self.validate_tls(server_name)
    }

    pub fn credential_source(&self) -> Result<Option<CredentialSource<'_>>> {
        let count = usize::from(self.api_key.is_some())
            + usize::from(self.api_key_env.is_some())
            + usize::from(self.api_key_keyring.is_some());
        match count {
            0 => Ok(None),
            1 => {
                if let Some(api_key) = self.api_key.as_deref() {
                    Ok(Some(CredentialSource::Inline(api_key)))
                } else if let Some(var_name) = self.api_key_env.as_deref() {
                    Ok(Some(CredentialSource::EnvVar(var_name)))
                } else {
                    let r = self.api_key_keyring.as_ref().ok_or_else(|| {
                        BzrError::config("internal: keyring credential unexpectedly missing")
                    })?;
                    // Empty string means "default to the server_name"; the
                    // real account is resolved in resolve_api_key() which
                    // has the server name in scope. We cannot use
                    // KeyringRef::account_or_default here because that would
                    // require plumbing the server name through every caller.
                    Ok(Some(CredentialSource::Keyring {
                        service: r.service_or_default(),
                        account: r.account.as_deref().unwrap_or(""),
                    }))
                }
            }
            _ => Err(BzrError::config(
                "server config cannot define multiple API key sources \
                 (api_key, api_key_env, api_key_keyring)",
            )),
        }
    }

    pub fn credential_source_kind(&self) -> Result<Option<CredentialSourceKind>> {
        Ok(self.credential_source()?.map(|source| source.kind()))
    }

    pub fn resolve_optional_api_key(&self, server_name: &str) -> Result<Option<String>> {
        match self.credential_source()? {
            Some(CredentialSource::Inline(api_key)) => Ok(Some(api_key.to_string())),
            Some(CredentialSource::EnvVar(var_name)) => {
                let value = std::env::var(var_name).map_err(|_| {
                    BzrError::config(format!(
                        "server '{server_name}' uses API key env var '{var_name}', but it is not set"
                    ))
                })?;
                if value.is_empty() {
                    return Err(BzrError::config(format!(
                        "server '{server_name}' uses API key env var '{var_name}', but it is empty"
                    )));
                }
                Ok(Some(value))
            }
            Some(CredentialSource::Keyring { service, account }) => {
                // Empty `account` means "default to server_name" (see the
                // sentinel explanation in credential_source()).
                let account = if account.is_empty() {
                    server_name
                } else {
                    account
                };
                crate::credentials::keyring::retrieve(service, account).map(Some)
            }
            None => Ok(None),
        }
    }

    pub fn resolve_api_key(&self, server_name: &str) -> Result<String> {
        self.resolve_optional_api_key(server_name)?.ok_or_else(|| {
            BzrError::config(format!(
                "server '{server_name}' has no API key source configured"
            ))
        })
    }

    pub fn validate_tls(&self, server_name: &str) -> Result<()> {
        let ctx = |msg: &str| BzrError::config(format!("server '{server_name}': {msg}"));

        if self.tls_insecure && self.tls_ca_cert.is_some() {
            return Err(ctx("tls_insecure and tls_ca_cert are mutually exclusive"));
        }
        if self.tls_insecure && self.tls_pin_sha256.is_some() {
            return Err(ctx(
                "tls_insecure and tls_pin_sha256 are mutually exclusive",
            ));
        }
        if self.tls_ca_cert.is_some() && self.tls_pin_sha256.is_some() {
            return Err(ctx("tls_ca_cert and tls_pin_sha256 are mutually exclusive"));
        }
        if let Some(path) = &self.tls_ca_cert {
            if !path.exists() {
                return Err(BzrError::config(format!(
                    "server '{server_name}': tls_ca_cert file not found: {}",
                    path.display()
                )));
            }
        }
        if let Some(pin) = &self.tls_pin_sha256 {
            crate::tls::fingerprint::parse_pin(pin)
                .map_err(|e| ctx(&format!("invalid tls_pin_sha256: {e}")))?;
        }
        Ok(())
    }
}

impl Config {
    /// Resolve the path to the config file.
    ///
    /// Precedence: the `--config <PATH>` override (set via
    /// [`Self::set_path_override`]) > the `BZR_CONFIG` environment variable
    /// (a full path to the config file) > `$XDG_CONFIG_HOME/bzr/config.toml` >
    /// the platform default config dir. The first two point directly at the
    /// file; the last two name a directory under which `bzr/config.toml` is
    /// used.
    pub fn path() -> Result<PathBuf> {
        if let Ok(guard) = CONFIG_PATH_OVERRIDE.read() {
            if let Some(path) = guard.as_ref() {
                return Ok(path.clone());
            }
        }
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

    /// Install (or clear) the `--config <PATH>` override that takes precedence
    /// over `BZR_CONFIG` and the default config directory.
    ///
    /// Called once from `main` after argument parsing. Passing `None` clears
    /// the override (used by tests to restore the default resolution).
    pub fn set_path_override(path: Option<PathBuf>) {
        if let Ok(mut guard) = CONFIG_PATH_OVERRIDE.write() {
            *guard = path;
        }
    }

    /// Resolve the config directory (`<config>/bzr`), creating it `0700` on
    /// first use, and return it. Shared by `write_to_disk` and `update_locked`.
    fn ensure_config_dir() -> Result<PathBuf> {
        let path = Self::path()?;
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
    /// `update_locked` (which validates the post-mutation state) and by `load`.
    ///
    /// `pub(crate)` so `config remove-server`/`rename-server` can take an
    /// advisory snapshot (existence, default pointer, keyring ref) even when
    /// some unrelated server has invalid fields.
    pub(crate) fn read_unvalidated() -> Result<Config> {
        let path = Self::path()?;
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

    pub fn load() -> Result<Config> {
        // Warn on insecure permissions only on an explicit load (preserves today's
        // behavior); `update_locked`'s internal reload uses `read_unvalidated`, so
        // it warns once (from `write_to_disk`) rather than twice.
        let path = Self::path()?;
        if path.exists() {
            Self::warn_on_insecure_permissions(&path);
        }
        let config = Self::read_unvalidated()?;
        config.validate()?;
        Ok(config)
    }

    #[cfg(test)]
    fn save(&self) -> Result<()> {
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
    pub fn update_locked(mutator: impl FnOnce(&mut Config) -> Result<()>) -> Result<Config> {
        Self::update_locked_inner(true, mutator)
    }

    /// Like [`Self::update_locked`] but skips whole-config validation for callers
    /// that preserve existing invalid config while changing unrelated data.
    pub fn update_locked_without_validation(
        mutator: impl FnOnce(&mut Config) -> Result<()>,
    ) -> Result<Config> {
        Self::update_locked_inner(false, mutator)
    }

    fn update_locked_inner(
        validate: bool,
        mutator: impl FnOnce(&mut Config) -> Result<()>,
    ) -> Result<Config> {
        if LOCK_HELD.with(Cell::get) {
            return Err(BzrError::config(
                "internal error: Config::update_locked called re-entrantly \
                 (a mutation closure must not write the config itself)",
            ));
        }

        let dir = Self::ensure_config_dir()?;
        let lock_path = dir.join("config.lock");
        let file = open_lock_file(&lock_path)?;
        acquire_exclusive_lock(&file, &lock_path)?;
        LOCK_HELD.with(|held| held.set(true));
        let _guard = LockGuard { file };

        // Reload WITHOUT validation. `Config::load` validates unconditionally;
        // validating the reload would make `update_locked_without_validation`
        // fail before a caller can repair or preserve unrelated invalid config.
        // We validate only the post-mutation state when `validate` is true,
        // matching `save()`'s "validate the whole config before writing"
        // semantics.
        let mut config = Self::read_unvalidated()?;
        mutator(&mut config)?;
        if validate {
            config.validate()?;
        }
        config.write_to_disk()?;
        Ok(config)
    }

    /// Persist the config **without** running validation.
    ///
    /// Used only in tests. Applies the same `0o600`/`0o700` hardening as `save`
    /// so a recreated config file is never world-readable.
    #[cfg(test)]
    fn save_without_validation(&self) -> Result<()> {
        self.write_to_disk()
    }

    /// Serialize and write the config to its on-disk path atomically:
    /// a uniquely-named sibling temp is written `0o600`, fsync'd (unix),
    /// renamed over the target (atomic replace), and the directory is
    /// fsync'd (unix) so the rename survives a crash. A concurrent reader
    /// therefore always sees either the complete old or complete new file.
    fn write_to_disk(&self) -> Result<()> {
        let _dir = Self::ensure_config_dir()?;
        let path = Self::path()?;
        reap_stale_temps(&path);
        let content = toml::to_string_pretty(self).map_err(|e| {
            BzrError::config(format!("serialize config file '{}': {e}", path.display()))
        })?;
        atomic_write(&path, &content)?;
        Self::warn_on_insecure_permissions(&path);
        Ok(())
    }

    pub fn resolve_server<'a>(
        &'a self,
        server_name: Option<&'a str>,
    ) -> Result<(&'a str, &'a ServerConfig)> {
        let name = self.resolve_server_name_only(server_name)?;
        let srv = self
            .servers
            .get(name)
            .ok_or_else(|| BzrError::config(format!("server '{name}' not found in config")))?;
        Ok((name, srv))
    }

    pub fn resolve_server_name_only<'a>(&'a self, server_name: Option<&'a str>) -> Result<&'a str> {
        server_name
            .or(self.default_server.as_deref())
            .ok_or_else(|| {
                BzrError::config(
                    "no server configured. Run `bzr config set-server <name> --url <url> --api-key-env <env-var>` first",
                )
            })
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

    fn validate(&self) -> Result<()> {
        for (name, server) in &self.servers {
            server.validate(name)?;
        }
        Ok(())
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
pub(crate) fn set_fail_after_temp(on: bool) {
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
fn fsync_parent_dir(path: &std::path::Path) -> Result<()> {
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

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
