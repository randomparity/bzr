//! Safe installation of the skill payload embedded in the running binary.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::hash::BuildHasher as _;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cli::AgentTarget;
use crate::commands::skills::InstallScope;
use crate::error::{BzrError, Result};
use crate::skills::embedded;

const SENTINEL: &str = ".bzr-skill-managed";
const LOCK_DIRECTORY: &str = ".bzr-skill.lock";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub(crate) struct InstallRequest {
    pub(crate) agent: AgentTarget,
    pub(crate) scope: InstallScope,
    pub(crate) home: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstalledDestination {
    pub(crate) layout: &'static str,
    pub(crate) path: PathBuf,
    pub(crate) installed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallOutcome {
    pub(crate) destinations: Vec<InstalledDestination>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedDestination {
    layout: &'static str,
    path: PathBuf,
}

trait FileOperations {
    fn create_dir(&mut self, path: &Path) -> std::io::Result<()>;
    fn write_new(&mut self, path: &Path, bytes: &[u8]) -> std::io::Result<()>;
    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()>;
    fn remove_dir_all(&mut self, path: &Path) -> std::io::Result<()>;
    fn before_final_target_check(&mut self, _target: &Path) -> std::io::Result<()> {
        Ok(())
    }
}

struct StdFileOperations;

impl FileOperations for StdFileOperations {
    fn create_dir(&mut self, path: &Path) -> std::io::Result<()> {
        fs::create_dir(path)
    }

    fn write_new(&mut self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(bytes)
    }

    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        fs::rename(from, to)
    }

    fn remove_dir_all(&mut self, path: &Path) -> std::io::Result<()> {
        fs::remove_dir_all(path)
    }
}

pub(crate) fn install(request: InstallRequest) -> Result<InstallOutcome> {
    install_with_ops(request, &mut StdFileOperations)
}

fn install_with_ops<O: FileOperations>(
    request: InstallRequest,
    ops: &mut O,
) -> Result<InstallOutcome> {
    let InstallRequest { agent, scope, home } = request;
    let destinations = destinations_for(agent, &scope, home.as_deref())?;
    let root = selected_root(&scope, home.as_deref())?;
    let skills = embedded::skill_names();

    for destination in &destinations {
        preflight_destination(root, &destination.path, &skills)?;
    }
    for destination in &destinations {
        ensure_destination(root, &destination.path, ops)?;
    }

    let mut locks = acquire_all_locks(&destinations, ops)?;
    if let Err(error) = authoritative_preflight(root, &destinations, &skills) {
        return Err(release_after_error(error, &mut locks, ops));
    }

    let mut installed_pairs = Vec::new();
    let mut installed_destinations = Vec::new();
    let mut warnings = Vec::new();
    for destination in &destinations {
        let mut installed = Vec::new();
        for skill in &skills {
            if let Err(error) = install_skill(destination, skill, ops, &mut warnings) {
                let error = append_installed(error, &installed_pairs);
                return Err(release_after_error(error, &mut locks, ops));
            }
            installed.push((*skill).to_string());
            installed_pairs.push(((*skill).to_string(), destination.path.clone()));
        }
        installed_destinations.push(InstalledDestination {
            layout: destination.layout,
            path: destination.path.clone(),
            installed,
        });
    }

    warnings.extend(release_all_locks(&mut locks, ops));
    Ok(InstallOutcome {
        destinations: installed_destinations,
        warnings,
    })
}

fn selected_root<'a>(scope: &'a InstallScope, home: Option<&'a Path>) -> Result<&'a Path> {
    match scope {
        InstallScope::Global => home.ok_or_else(|| {
            BzrError::input(
                "could not resolve a home directory for global skill installation".into(),
            )
        }),
        InstallScope::Project(project) => Ok(project),
    }
}

fn destinations_for(
    agent: AgentTarget,
    scope: &InstallScope,
    home: Option<&Path>,
) -> Result<Vec<ResolvedDestination>> {
    let root = selected_root(scope, home)?;
    let mut destinations = Vec::new();
    match agent {
        AgentTarget::Standard | AgentTarget::Bob | AgentTarget::Codex => {
            destinations.push(ResolvedDestination {
                layout: "agents",
                path: root.join(".agents/skills"),
            });
        }
        AgentTarget::Claude => destinations.push(ResolvedDestination {
            layout: "claude",
            path: root.join(".claude/skills"),
        }),
        AgentTarget::All => {
            destinations.push(ResolvedDestination {
                layout: "agents",
                path: root.join(".agents/skills"),
            });
            destinations.push(ResolvedDestination {
                layout: "claude",
                path: root.join(".claude/skills"),
            });
        }
    }
    Ok(destinations)
}

fn preflight_destination(root: &Path, destination: &Path, skills: &[&str]) -> Result<()> {
    validate_components(root, destination)?;
    if !destination.exists() {
        return Ok(());
    }
    for skill in skills {
        validate_target(&destination.join(skill), skill)?;
    }
    Ok(())
}

fn authoritative_preflight(
    root: &Path,
    destinations: &[ResolvedDestination],
    skills: &[&str],
) -> Result<()> {
    for destination in destinations {
        validate_components(root, &destination.path)?;
        for skill in skills {
            validate_target(&destination.path.join(skill), skill)?;
        }
    }
    Ok(())
}

fn validate_components(root: &Path, destination: &Path) -> Result<()> {
    let relative = destination.strip_prefix(root).map_err(|_| {
        BzrError::DataIntegrity(format!(
            "skill destination '{}' escapes selected root '{}'",
            destination.display(),
            root.display()
        ))
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(BzrError::DataIntegrity(format!(
                "skill destination is not normalized: '{}'",
                destination.display()
            )));
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(BzrError::DataIntegrity(format!(
                    "refusing skill destination '{}': component '{}' is a symbolic link",
                    destination.display(),
                    current.display()
                )));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(BzrError::DataIntegrity(format!(
                    "refusing skill destination '{}': component '{}' is not a directory",
                    destination.display(),
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(io_error("inspect destination component", &current, error)),
        }
    }
    Ok(())
}

fn ensure_destination<O: FileOperations>(
    root: &Path,
    destination: &Path,
    ops: &mut O,
) -> Result<()> {
    let relative = destination.strip_prefix(root).map_err(|_| {
        BzrError::DataIntegrity(format!(
            "destination escaped root: '{}'",
            destination.display()
        ))
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(BzrError::DataIntegrity(format!(
                "destination is not normalized: '{}'",
                destination.display()
            )));
        };
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(BzrError::DataIntegrity(format!(
                    "refusing destination component changed during creation: '{}'",
                    current.display()
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                match ops.create_dir(&current) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        validate_concurrently_created_directory(&current)?;
                    }
                    Err(error) => {
                        return Err(io_error("create destination directory", &current, error));
                    }
                }
            }
            Err(error) => return Err(io_error("inspect destination component", &current, error)),
        }
    }
    Ok(())
}

fn validate_concurrently_created_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error("inspect concurrently created directory", path, error))?;
    if metadata.file_type().is_symlink() {
        return Err(BzrError::DataIntegrity(format!(
            "refusing concurrently created destination '{}': path is a symbolic link",
            path.display()
        )));
    }
    if !metadata.is_dir() {
        return Err(BzrError::DataIntegrity(format!(
            "refusing concurrently created destination '{}': path is not a directory",
            path.display()
        )));
    }
    Ok(())
}

fn validate_target(target: &Path, skill: &str) -> Result<()> {
    let metadata = match fs::symlink_metadata(target) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error("inspect skill target", target, error)),
    };
    if metadata.file_type().is_symlink() {
        return Err(BzrError::DataIntegrity(format!(
            "refusing skill target '{}': target is a symbolic link",
            target.display()
        )));
    }
    if !metadata.is_dir() || !has_valid_sentinel(target, skill) {
        return Err(BzrError::DataIntegrity(format!(
            "refusing foreign skill target '{}': ownership sentinel is invalid",
            target.display()
        )));
    }
    Ok(())
}

fn has_valid_sentinel(target: &Path, skill: &str) -> bool {
    let path = target.join(SENTINEL);
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        return false;
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return false;
    }
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    parse_sentinel(&bytes, skill)
}

fn parse_sentinel(bytes: &[u8], skill: &str) -> bool {
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let mut fields = BTreeMap::new();
    let mut lines = text.split('\n').peekable();
    while let Some(line) = lines.next() {
        if !parse_sentinel_line(line, lines.peek().is_some(), &mut fields) {
            return false;
        }
    }
    required_sentinel_fields_are_valid(&fields, skill)
}

fn parse_sentinel_line<'a>(
    raw: &'a str,
    has_following_line: bool,
    fields: &mut BTreeMap<&'a str, &'a str>,
) -> bool {
    if raw.is_empty() {
        return !has_following_line;
    }
    let Some(line) = normalize_sentinel_line(raw, has_following_line) else {
        return false;
    };
    let Some((key, value)) = line.split_once(": ") else {
        return false;
    };
    if key.is_empty() || key.contains(':') || value.is_empty() {
        return false;
    }
    fields.insert(key, value).is_none()
}

fn normalize_sentinel_line(raw: &str, has_following_line: bool) -> Option<&str> {
    if raw.contains('\r') {
        return raw.strip_suffix('\r').filter(|_| has_following_line);
    }
    Some(raw)
}

fn required_sentinel_fields_are_valid(fields: &BTreeMap<&str, &str>, skill: &str) -> bool {
    let required = [
        "managed-by",
        "installed-skill",
        "source-version",
        "source-commit",
    ];
    required
        .iter()
        .all(|key| required_sentinel_value_is_valid(fields, key))
        && fields.get("managed-by") == Some(&"bzr-skill")
        && fields.get("installed-skill") == Some(&skill)
}

fn required_sentinel_value_is_valid(fields: &BTreeMap<&str, &str>, key: &str) -> bool {
    fields
        .get(key)
        .is_some_and(|value| value.is_ascii() && !value.is_empty())
}

struct DestinationLock {
    path: PathBuf,
    pid_file: Option<File>,
    owned: bool,
}

impl DestinationLock {
    #[cfg(test)]
    fn path(&self) -> &Path {
        &self.path
    }

    fn release<O: FileOperations>(&mut self, ops: &mut O) -> std::io::Result<()> {
        let cleanup = lock_cleanup_path(&self.path)?;
        if let Err(error) = fs::rename(&self.path, &cleanup) {
            self.pid_file.take();
            self.owned = false;
            return Err(error);
        }
        self.path = cleanup;
        self.pid_file.take();
        self.owned = false;
        ops.remove_dir_all(&self.path)
    }
}

impl Drop for DestinationLock {
    fn drop(&mut self) {
        if self.owned {
            let cleanup = lock_cleanup_path(&self.path);
            if let Ok(cleanup) = cleanup {
                if fs::rename(&self.path, &cleanup).is_ok() {
                    self.path = cleanup;
                }
            }
            self.pid_file.take();
            self.owned = false;
            if self.path.file_name().is_some_and(|name| {
                name.to_string_lossy()
                    .starts_with(".bzr-skill.lock.release.")
            }) {
                let _ = fs::remove_dir_all(&self.path);
            }
        }
    }
}

fn lock_cleanup_path(lock_path: &Path) -> std::io::Result<PathBuf> {
    let parent = lock_path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("destination lock has no parent: '{}'", lock_path.display()),
        )
    })?;
    Ok(temporary_path(parent, "lock", "release"))
}

fn acquire_destination_lock(destination: &Path) -> Result<DestinationLock> {
    let lock_path = destination.join(LOCK_DIRECTORY);
    match fs::create_dir(&lock_path) {
        Ok(()) => create_lock_file(lock_path),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            recover_or_refuse_existing_lock(&lock_path)?;
            fs::create_dir(&lock_path)
                .map_err(|error| io_error("acquire destination lock", &lock_path, error))?;
            create_lock_file(lock_path)
        }
        Err(error) => Err(io_error("acquire destination lock", &lock_path, error)),
    }
}

fn create_lock_file(lock_path: PathBuf) -> Result<DestinationLock> {
    let pid_path = lock_path.join("pid");
    let result = (|| -> std::io::Result<File> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&pid_path)?;
        file.lock()?;
        writeln!(file, "{}", std::process::id())?;
        file.sync_data()?;
        Ok(file)
    })();
    match result {
        Ok(pid_file) => Ok(DestinationLock {
            path: lock_path,
            pid_file: Some(pid_file),
            owned: true,
        }),
        Err(error) => {
            let _ = fs::remove_dir_all(&lock_path);
            Err(io_error("publish destination lock", &pid_path, error))
        }
    }
}

fn recover_or_refuse_existing_lock(lock_path: &Path) -> Result<()> {
    let lock_metadata = fs::symlink_metadata(lock_path)
        .map_err(|error| io_error("inspect destination lock", lock_path, error))?;
    if lock_metadata.file_type().is_symlink() || !lock_metadata.is_dir() {
        return Err(BzrError::DataIntegrity(format!(
            "refusing destination lock '{}': lock path is a symbolic link or not a directory",
            lock_path.display()
        )));
    }
    validate_stale_lock_entries(lock_path)?;
    let pid_path = lock_path.join("pid");
    let pid_metadata = fs::symlink_metadata(&pid_path).map_err(|error| {
        BzrError::DataIntegrity(format!(
            "destination is locked at '{}'; PID file '{}' is missing or unreadable: {error}; \
             remove the lock only after verifying no bzr skills install process is using it",
            lock_path.display(),
            pid_path.display()
        ))
    })?;
    if pid_metadata.file_type().is_symlink() || !pid_metadata.is_file() {
        return Err(BzrError::DataIntegrity(format!(
            concat!(
                "refusing destination lock '{}': PID file '{}' is a symbolic link ",
                "or not a regular file"
            ),
            lock_path.display(),
            pid_path.display()
        )));
    }
    let bytes = fs::read(&pid_path).map_err(|error| {
        BzrError::DataIntegrity(format!(
            "destination is locked at '{}'; PID file '{}' is missing or unreadable: {error}; \
             remove the lock only after verifying no bzr skills install process is using it",
            lock_path.display(),
            pid_path.display()
        ))
    })?;
    let pid = std::str::from_utf8(&bytes)
        .ok()
        .map(str::trim_end)
        .filter(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .ok_or_else(|| {
            BzrError::DataIntegrity(format!(
                "destination lock '{}' has an empty or malformed PID; remove it only after \
                 verifying no bzr skills install process is using it",
                lock_path.display()
            ))
        })?;
    let _pid: u32 = pid.parse().map_err(|_| {
        BzrError::DataIntegrity(format!(
            "destination lock '{}' has an invalid PID",
            lock_path.display()
        ))
    })?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&pid_path)
        .map_err(|error| io_error("inspect destination lock", &pid_path, error))?;
    match file.try_lock() {
        Err(TryLockError::WouldBlock) => Err(BzrError::DataIntegrity(format!(
            "destination is locked at '{}' by a live bzr process",
            lock_path.display()
        ))),
        Err(TryLockError::Error(error)) => {
            Err(io_error("inspect destination lock", &pid_path, error))
        }
        Ok(()) => {
            validate_stale_lock_entries(lock_path)?;
            let cleanup = lock_cleanup_path(lock_path)
                .map_err(|error| io_error("prepare stale-lock cleanup", lock_path, error))?;
            fs::rename(lock_path, &cleanup)
                .map_err(|error| io_error("detach stale destination lock", lock_path, error))?;
            drop(file);
            validate_stale_lock_entries(&cleanup)?;
            fs::remove_file(cleanup.join("pid"))
                .map_err(|error| io_error("remove stale destination lock PID", &cleanup, error))?;
            fs::remove_dir(&cleanup)
                .map_err(|error| io_error("remove stale destination lock", &cleanup, error))
        }
    }
}

fn validate_stale_lock_entries(lock_path: &Path) -> Result<()> {
    let entries = fs::read_dir(lock_path)
        .map_err(|error| io_error("inspect destination lock entries", lock_path, error))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| io_error("inspect destination lock entry", lock_path, error))?;
        if entry.file_name() != "pid" {
            return Err(BzrError::DataIntegrity(format!(
                "destination lock '{}' has unexpected entry '{}'; preserve it and remove the lock \
                 only after verifying no bzr skills install process is using it",
                lock_path.display(),
                entry.path().display()
            )));
        }
    }
    Ok(())
}

fn acquire_all_locks<O: FileOperations>(
    destinations: &[ResolvedDestination],
    ops: &mut O,
) -> Result<Vec<DestinationLock>> {
    let mut paths: Vec<_> = destinations.iter().map(|item| &item.path).collect();
    paths.sort();
    let mut locks = Vec::new();
    for path in paths {
        match acquire_destination_lock(path) {
            Ok(lock) => locks.push(lock),
            Err(error) => return Err(release_after_error(error, &mut locks, ops)),
        }
    }
    Ok(locks)
}

fn install_skill<O: FileOperations>(
    destination: &ResolvedDestination,
    skill: &str,
    ops: &mut O,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let target = destination.path.join(skill);
    let stage = temporary_path(&destination.path, "stage", skill);
    fs::create_dir(&stage).map_err(|error| io_error("create skill stage", &stage, error))?;
    if let Err(error) = write_staged_skill(&stage, skill, ops) {
        return Err(clean_stage_after_error(error, &stage, ops));
    }
    if let Err(error) = ops.before_final_target_check(&target) {
        let error = io_error("prepare final skill target check", &target, error);
        return Err(clean_stage_after_error(error, &stage, ops));
    }
    if let Err(error) = validate_target(&target, skill) {
        return Err(clean_stage_after_error(error, &stage, ops));
    }
    replace_target(&target, &stage, skill, ops, warnings)
}

fn write_staged_skill<O: FileOperations>(stage: &Path, skill: &str, ops: &mut O) -> Result<()> {
    for file in embedded::files() {
        let Some(relative) = file.relative_path.strip_prefix(skill) else {
            continue;
        };
        let Some(relative) = relative.strip_prefix('/') else {
            continue;
        };
        let path = stage.join(relative);
        if let Some(parent) = path.parent() {
            ensure_stage_parent(stage, parent)?;
        }
        ops.write_new(&path, file.bytes)
            .map_err(|error| io_error("write staged skill file", &path, error))?;
    }
    let sentinel_path = stage.join(SENTINEL);
    let contents = format!(
        "managed-by: bzr-skill\ninstalled-skill: {skill}\nsource-version: {}\nsource-commit: {}\n",
        env!("CARGO_PKG_VERSION"),
        env!("BZR_GIT_SHA")
    );
    ops.write_new(&sentinel_path, contents.as_bytes())
        .map_err(|error| io_error("write ownership sentinel", &sentinel_path, error))?;
    let entrypoint = stage.join("SKILL.md");
    let metadata = fs::symlink_metadata(&entrypoint)
        .map_err(|error| io_error("verify staged SKILL.md", &entrypoint, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(BzrError::DataIntegrity(format!(
            "staged skill is missing a regular SKILL.md: '{}'",
            entrypoint.display()
        )));
    }
    Ok(())
}

fn ensure_stage_parent(stage: &Path, parent: &Path) -> Result<()> {
    let relative = parent.strip_prefix(stage).map_err(|_| {
        BzrError::DataIntegrity(format!("staged path escaped stage: '{}'", parent.display()))
    })?;
    let mut current = stage.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(BzrError::DataIntegrity(format!(
                "embedded path is not normalized: '{}'",
                relative.display()
            )));
        };
        current.push(component);
        match fs::create_dir(&current) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let metadata = fs::symlink_metadata(&current)
                    .map_err(|error| io_error("inspect staged directory", &current, error))?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(BzrError::DataIntegrity(format!(
                        "staged path is not a directory: '{}'",
                        current.display()
                    )));
                }
            }
            Err(error) => return Err(io_error("create staged directory", &current, error)),
        }
    }
    Ok(())
}

fn replace_target<O: FileOperations>(
    target: &Path,
    stage: &Path,
    skill: &str,
    ops: &mut O,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let exists = fs::symlink_metadata(target).is_ok();
    if !exists {
        return ops
            .rename(stage, target)
            .map_err(|error| io_error("activate staged skill", target, error))
            .map_err(|error| clean_stage_after_error(error, stage, ops));
    }

    let parent = target.parent().ok_or_else(|| {
        BzrError::DataIntegrity(format!(
            "skill target has no parent: '{}'",
            target.display()
        ))
    })?;
    let aside = temporary_path(parent, "old", skill);
    if let Err(error) = ops.rename(target, &aside) {
        let error = io_error("move existing skill aside", target, error);
        return Err(clean_stage_after_error(error, stage, ops));
    }
    if let Err(error) = validate_target(&aside, skill) {
        return Err(restore_detached_target(
            error,
            &DetachedTarget {
                target,
                stage,
                aside: &aside,
            },
            "restored foreign content",
            ops,
        ));
    }
    if let Err(error) = ops.rename(stage, target) {
        let activation = io_error("activate staged skill", target, error);
        return Err(restore_detached_target(
            activation,
            &DetachedTarget {
                target,
                stage,
                aside: &aside,
            },
            "restored previous content",
            ops,
        ));
    }
    if let Err(error) = ops.remove_dir_all(&aside) {
        warnings.push(format!(
            "installed '{skill}' at '{}' but could not remove residual aside '{}': {error}; \
             verify the installed target, then remove the aside",
            target.display(),
            aside.display()
        ));
    }
    Ok(())
}

struct DetachedTarget<'a> {
    target: &'a Path,
    stage: &'a Path,
    aside: &'a Path,
}

fn restore_detached_target<O: FileOperations>(
    failure: BzrError,
    detached: &DetachedTarget<'_>,
    restored_detail: &str,
    ops: &mut O,
) -> BzrError {
    match ops.rename(detached.aside, detached.target) {
        Ok(()) => {
            let error = append_detail(failure, restored_detail);
            clean_stage_after_error(error, detached.stage, ops)
        }
        Err(restore_error) => append_detail(
            failure,
            &format!(
                "restore failed: {restore_error}; detached content remains at '{}'; staged \
                 content remains at '{}'",
                detached.aside.display(),
                detached.stage.display()
            ),
        ),
    }
}

fn clean_stage_after_error<O: FileOperations>(
    error: BzrError,
    stage: &Path,
    ops: &mut O,
) -> BzrError {
    match ops.remove_dir_all(stage) {
        Ok(()) => error,
        Err(cleanup_error) => append_detail(
            error,
            &format!(
                "could not remove residual stage '{}': {cleanup_error}",
                stage.display()
            ),
        ),
    }
}

fn temporary_path(parent: &Path, kind: &str, skill: &str) -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let token = std::collections::hash_map::RandomState::new().hash_one((
        std::process::id(),
        sequence,
        std::time::SystemTime::now(),
    ));
    parent.join(format!(
        ".bzr-skill.{kind}.{skill}.{}.{token:016x}",
        std::process::id(),
    ))
}

fn release_all_locks<O: FileOperations>(locks: &mut [DestinationLock], ops: &mut O) -> Vec<String> {
    let mut warnings = Vec::new();
    for lock in locks.iter_mut().rev() {
        if let Err(error) = lock.release(ops) {
            let path = lock.path.clone();
            warnings.push(format!(
                "installation completed but could not release lock '{}': {error}; verify no bzr \
                 skills install process is using it before removing it",
                path.display()
            ));
        }
    }
    warnings.sort();
    warnings
}

fn release_after_error<O: FileOperations>(
    mut error: BzrError,
    locks: &mut [DestinationLock],
    ops: &mut O,
) -> BzrError {
    for warning in release_all_locks(locks, ops) {
        error = append_detail(error, &warning);
    }
    error
}

fn append_installed(error: BzrError, installed: &[(String, PathBuf)]) -> BzrError {
    let mut pairs = installed.to_vec();
    pairs.sort();
    let detail = if pairs.is_empty() {
        "installed before failure: none".to_string()
    } else {
        let pairs = pairs
            .iter()
            .map(|(skill, destination)| format!("{skill} -> {}", destination.display()))
            .collect::<Vec<_>>()
            .join(", ");
        format!("installed before failure: {pairs}")
    };
    append_detail(error, &detail)
}

fn append_detail(error: BzrError, detail: &str) -> BzrError {
    match error {
        BzrError::Io(error) => BzrError::Io(std::io::Error::new(
            error.kind(),
            format!("{error}; {detail}"),
        )),
        BzrError::DataIntegrity(message) => BzrError::DataIntegrity(format!("{message}; {detail}")),
        BzrError::InputValidation {
            message,
            field,
            value,
        } => BzrError::InputValidation {
            message: format!("{message}; {detail}"),
            field,
            value,
        },
        other => BzrError::DataIntegrity(format!("{other}; {detail}")),
    }
}

fn io_error(operation: &str, path: &Path, error: std::io::Error) -> BzrError {
    let kind = error.kind();
    let message = error.to_string();
    drop(error);
    BzrError::Io(std::io::Error::new(
        kind,
        format!("{operation} '{}': {message}", path.display()),
    ))
}

#[cfg(feature = "test-helpers")]
pub(crate) fn hold_destination_lock(path: &Path, ready: &Path, release: &Path) -> Result<()> {
    let _guard = acquire_destination_lock(path)?;
    let ready_stage = ready.with_extension(format!("tmp.{}", std::process::id()));
    fs::write(&ready_stage, b"ready\n")
        .map_err(|error| io_error("write lock-helper readiness", &ready_stage, error))?;
    fs::rename(&ready_stage, ready)
        .map_err(|error| io_error("publish lock-helper readiness", ready, error))?;
    let step = std::time::Duration::from_millis(10);
    let mut remaining = std::time::Duration::from_secs(30);
    while !release.exists() {
        if remaining.is_zero() {
            return Err(BzrError::Io(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "timed out waiting for lock-helper release '{}'",
                    release.display()
                ),
            )));
        }
        std::thread::sleep(step);
        remaining = remaining.saturating_sub(step);
    }
    Ok(())
}

#[cfg(test)]
#[path = "installer_tests.rs"]
mod tests;
