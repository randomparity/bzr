#![expect(clippy::panic, clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use super::{
    acquire_destination_lock, destinations_for, install_with_ops, DestinationLock, FileOperations,
    InstallRequest,
};
use crate::cli::AgentTarget;
use crate::commands::skills::InstallScope;
use crate::skills::embedded;

const SKILL: &str = "bzr-bulk-triage";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Failure {
    None,
    Write,
    TargetToAside,
    Activate,
    Restore,
    RemoveAside,
    RemoveStage,
    ReleaseLock,
    ReleaseHandoff,
    RaceForeign,
}

struct TestOps {
    failures: BTreeSet<Failure>,
    triggered: BTreeSet<Failure>,
    successor: Option<DestinationLock>,
}

impl TestOps {
    fn new(failure: Failure) -> Self {
        if failure == Failure::None {
            return Self::with_failures(&[]);
        }
        Self::with_failures(&[failure])
    }

    fn with_failures(failures: &[Failure]) -> Self {
        Self {
            failures: failures.iter().copied().collect(),
            triggered: BTreeSet::new(),
            successor: None,
        }
    }

    fn fail_once(&mut self, failure: Failure) -> std::io::Result<()> {
        if self.failures.contains(&failure) && self.triggered.insert(failure) {
            return Err(std::io::Error::other(format!("injected {failure:?}")));
        }
        Ok(())
    }
}

impl FileOperations for TestOps {
    fn write_new(&mut self, path: &Path, bytes: &[u8]) -> std::io::Result<()> {
        self.fail_once(Failure::Write)?;
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(bytes)
    }

    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        let from_name = from
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let to_name = to.file_name().and_then(|name| name.to_str()).unwrap_or("");
        if to_name.contains(".bzr-skill.old.") {
            self.fail_once(Failure::TargetToAside)?;
        } else if from_name.contains(".bzr-skill.stage.") {
            self.fail_once(Failure::Activate)?;
        } else if from_name.contains(".bzr-skill.old.") {
            self.fail_once(Failure::Restore)?;
        }
        fs::rename(from, to)
    }

    fn remove_dir_all(&mut self, path: &Path) -> std::io::Result<()> {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if self.failures.contains(&Failure::ReleaseHandoff)
            && self.triggered.insert(Failure::ReleaseHandoff)
        {
            let destination = path
                .parent()
                .ok_or_else(|| std::io::Error::other("lock cleanup path has no parent"))?;
            self.successor = Some(
                acquire_destination_lock(destination)
                    .map_err(|error| std::io::Error::other(error.to_string()))?,
            );
        }
        if name.contains(".bzr-skill.old.") {
            self.fail_once(Failure::RemoveAside)?;
        } else if name.contains(".bzr-skill.stage.") {
            self.fail_once(Failure::RemoveStage)?;
        } else if name.starts_with(".bzr-skill.lock") {
            self.fail_once(Failure::ReleaseLock)?;
        }
        fs::remove_dir_all(path)
    }

    fn before_final_target_check(&mut self, target: &Path) -> std::io::Result<()> {
        if self.failures.contains(&Failure::RaceForeign)
            && self.triggered.insert(Failure::RaceForeign)
            && target.ends_with(SKILL)
        {
            if target.exists() {
                fs::remove_dir_all(target)?;
            }
            fs::create_dir(target)?;
            fs::write(target.join("foreign.txt"), b"foreign bytes")?;
        }
        Ok(())
    }
}

fn request(root: &Path, agent: AgentTarget) -> InstallRequest {
    InstallRequest {
        agent,
        scope: InstallScope::Project(root.to_path_buf()),
        home: None,
    }
}

fn destination(root: &Path) -> PathBuf {
    root.join(".agents/skills")
}

fn target(root: &Path) -> PathBuf {
    destination(root).join(SKILL)
}

fn sentinel(skill: &str) -> String {
    format!(
        "managed-by: bzr-skill\ninstalled-skill: {skill}\nsource-version: test\nsource-commit: abc\n"
    )
}

fn create_owned(root: &Path, skill: &str) -> PathBuf {
    let target = destination(root).join(skill);
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join(".bzr-skill-managed"), sentinel(skill)).unwrap();
    fs::write(target.join("old.txt"), b"old bytes").unwrap();
    target
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, (String, Vec<u8>)> {
    fn walk(root: &Path, path: &Path, snapshot: &mut BTreeMap<PathBuf, (String, Vec<u8>)>) {
        let mut entries = fs::read_dir(path)
            .unwrap()
            .collect::<std::io::Result<Vec<_>>>()
            .unwrap();
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap().to_path_buf();
            let metadata = fs::symlink_metadata(&path).unwrap();
            if metadata.file_type().is_symlink() {
                snapshot.insert(
                    relative,
                    (
                        "symlink".into(),
                        fs::read_link(&path)
                            .unwrap()
                            .as_os_str()
                            .as_encoded_bytes()
                            .to_vec(),
                    ),
                );
            } else if metadata.is_dir() {
                snapshot.insert(relative, ("dir".into(), Vec::new()));
                walk(root, &path, snapshot);
            } else {
                snapshot.insert(relative, ("file".into(), fs::read(&path).unwrap()));
            }
        }
    }

    let mut result = BTreeMap::new();
    walk(root, root, &mut result);
    result
}

fn run(root: &Path, failure: Failure) -> crate::error::Result<super::InstallOutcome> {
    install_with_ops(
        request(root, AgentTarget::Standard),
        &mut TestOps::with_failures(&[failure]),
    )
}

#[test]
fn maps_every_agent_for_project_and_global_scopes_without_duplicate_all_destinations() {
    let root = Path::new("/tmp/example-root");
    for scope in [
        InstallScope::Project(root.to_path_buf()),
        InstallScope::Global,
    ] {
        for agent in [AgentTarget::Standard, AgentTarget::Bob, AgentTarget::Codex] {
            let destinations = destinations_for(agent, &scope, Some(root)).unwrap();
            assert_eq!(destinations.len(), 1);
            assert_eq!(destinations[0].layout, "agents");
            assert_eq!(destinations[0].path, root.join(".agents/skills"));
        }
        let claude = destinations_for(AgentTarget::Claude, &scope, Some(root)).unwrap();
        assert_eq!(claude.len(), 1);
        assert_eq!(claude[0].layout, "claude");
        let all = destinations_for(AgentTarget::All, &scope, Some(root)).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].layout, "agents");
        assert_eq!(all[1].layout, "claude");
    }
}

#[test]
fn global_scope_requires_a_home_directory() {
    let error = destinations_for(AgentTarget::Codex, &InstallScope::Global, None).unwrap_err();
    assert!(error.to_string().contains("home directory"));
}

#[test]
fn installs_all_embedded_files_and_replaces_owned_content_idempotently() {
    let temp = tempfile::TempDir::new().unwrap();
    create_owned(temp.path(), SKILL);

    let first = run(temp.path(), Failure::None).unwrap();
    let second = run(temp.path(), Failure::None).unwrap();

    assert_eq!(first.destinations[0].installed, embedded::skill_names());
    assert_eq!(second.destinations, first.destinations);
    assert!(target(temp.path()).join("SKILL.md").is_file());
    assert!(target(temp.path()).join(".bzr-skill-managed").is_file());
    assert!(!target(temp.path()).join("old.txt").exists());
}

#[test]
fn accepts_posix_lf_and_powershell_bom_crlf_sentinels_with_unknown_fields() {
    for bytes in [
        sentinel(SKILL).into_bytes(),
        format!(
            "\u{feff}managed-by: bzr-skill\r\ninstalled-skill: {SKILL}\r\nsource-version: test\r\nsource-commit: abc\r\nfuture-field: accepted\r\n"
        )
        .into_bytes(),
    ] {
        let temp = tempfile::TempDir::new().unwrap();
        let target = create_owned(temp.path(), SKILL);
        fs::write(target.join(".bzr-skill-managed"), bytes).unwrap();
        run(temp.path(), Failure::None).unwrap();
        assert!(target.join("SKILL.md").is_file());
    }
}

#[test]
fn refuses_every_malformed_or_mismatched_ownership_sentinel_without_tree_changes() {
    let cases = [
        ("missing", None),
        ("empty", Some("")),
        (
            "wrong-manager",
            Some("managed-by: other\ninstalled-skill: bzr-bulk-triage\nsource-version: x\nsource-commit: y\n"),
        ),
        (
            "duplicate-manager",
            Some("managed-by: bzr-skill\nmanaged-by: bzr-skill\ninstalled-skill: bzr-bulk-triage\nsource-version: x\nsource-commit: y\n"),
        ),
        (
            "missing-field",
            Some("managed-by: bzr-skill\ninstalled-skill: bzr-bulk-triage\nsource-version: x\n"),
        ),
        (
            "empty-field",
            Some("managed-by: bzr-skill\ninstalled-skill: bzr-bulk-triage\nsource-version: \nsource-commit: y\n"),
        ),
        (
            "wrong-skill",
            Some("managed-by: bzr-skill\ninstalled-skill: other\nsource-version: x\nsource-commit: y\n"),
        ),
        (
            "malformed",
            Some("managed-by:bzr-skill\ninstalled-skill: bzr-bulk-triage\nsource-version: x\nsource-commit: y\n"),
        ),
        (
            "extra-trailing-newline",
            Some("managed-by: bzr-skill\ninstalled-skill: bzr-bulk-triage\nsource-version: x\nsource-commit: y\n\n"),
        ),
        (
            "bare-trailing-carriage-return",
            Some("managed-by: bzr-skill\ninstalled-skill: bzr-bulk-triage\nsource-version: x\nsource-commit: y\r"),
        ),
    ];

    for (name, contents) in cases {
        let temp = tempfile::TempDir::new().unwrap();
        let target = create_owned(temp.path(), SKILL);
        let sentinel_path = target.join(".bzr-skill-managed");
        if let Some(contents) = contents {
            fs::write(&sentinel_path, contents).unwrap();
        } else {
            fs::remove_file(&sentinel_path).unwrap();
        }
        let before = snapshot(temp.path());
        let error = run(temp.path(), Failure::None).unwrap_err();
        assert!(error.to_string().contains("foreign"), "{name}: {error}");
        assert_eq!(snapshot(temp.path()), before, "{name}");
    }
}

#[cfg(unix)]
#[test]
fn refuses_symlink_and_directory_sentinels_without_tree_changes() {
    use std::os::unix::fs::symlink;

    for sentinel_kind in ["symlink", "directory"] {
        let temp = tempfile::TempDir::new().unwrap();
        let target = create_owned(temp.path(), SKILL);
        let sentinel_path = target.join(".bzr-skill-managed");
        fs::remove_file(&sentinel_path).unwrap();
        if sentinel_kind == "symlink" {
            fs::write(target.join("sentinel-source"), sentinel(SKILL)).unwrap();
            symlink("sentinel-source", &sentinel_path).unwrap();
        } else {
            fs::create_dir(&sentinel_path).unwrap();
        }
        let before = snapshot(temp.path());
        assert!(run(temp.path(), Failure::None).is_err());
        assert_eq!(snapshot(temp.path()), before);
    }
}

#[cfg(unix)]
#[test]
fn refuses_symlinks_at_each_destination_component_and_target() {
    use std::os::unix::fs::symlink;

    for component in [
        ".agents",
        ".agents/skills",
        ".agents/skills/bzr-bulk-triage",
    ] {
        let temp = tempfile::TempDir::new().unwrap();
        let external = tempfile::TempDir::new().unwrap();
        let link = temp.path().join(component);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(external.path(), &link).unwrap();
        let before = snapshot(temp.path());
        let error = run(temp.path(), Failure::None).unwrap_err();
        assert!(
            error.to_string().contains("symbolic link"),
            "{component}: {error}"
        );
        assert_eq!(snapshot(temp.path()), before);
    }
}

#[cfg(unix)]
#[test]
fn refuses_symlinks_in_the_claude_destination_layout() {
    use std::os::unix::fs::symlink;

    for component in [
        ".claude",
        ".claude/skills",
        ".claude/skills/bzr-bulk-triage",
    ] {
        let temp = tempfile::TempDir::new().unwrap();
        let external = tempfile::TempDir::new().unwrap();
        let link = temp.path().join(component);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        symlink(external.path(), &link).unwrap();
        let before = snapshot(temp.path());
        let error = install_with_ops(
            request(temp.path(), AgentTarget::Claude),
            &mut TestOps::new(Failure::None),
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("symbolic link"),
            "{component}: {error}"
        );
        assert_eq!(snapshot(temp.path()), before);
    }
}

#[cfg(unix)]
#[test]
fn refuses_an_unreadable_ownership_sentinel_and_preserves_its_bytes() {
    use std::os::unix::fs::PermissionsExt as _;

    let temp = tempfile::TempDir::new().unwrap();
    let target = create_owned(temp.path(), SKILL);
    let sentinel_path = target.join(".bzr-skill-managed");
    let original = fs::read(&sentinel_path).unwrap();
    fs::set_permissions(&sentinel_path, fs::Permissions::from_mode(0o000)).unwrap();

    let result = run(temp.path(), Failure::None);

    let mode = fs::symlink_metadata(&sentinel_path)
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    fs::set_permissions(&sentinel_path, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(result.is_err());
    assert_eq!(mode, 0);
    assert_eq!(fs::read(&sentinel_path).unwrap(), original);
    assert_eq!(fs::read(target.join("old.txt")).unwrap(), b"old bytes");
}

#[test]
fn lock_protocol_handles_absent_transient_live_dead_and_malformed_pid_files() {
    let temp = tempfile::TempDir::new().unwrap();
    let destination = temp.path().join("skills");
    fs::create_dir(&destination).unwrap();
    let mut ops = TestOps::new(Failure::None);

    let mut guard = acquire_destination_lock(&destination).unwrap();
    assert!(guard.path().join("pid").is_file());
    assert!(acquire_destination_lock(&destination).is_err());
    guard.release(&mut ops).unwrap();

    let lock = destination.join(".bzr-skill.lock");
    fs::create_dir(&lock).unwrap();
    assert!(acquire_destination_lock(&destination).is_err());
    fs::write(lock.join("pid"), b"").unwrap();
    assert!(acquire_destination_lock(&destination).is_err());
    fs::write(lock.join("pid"), b"not-a-pid\n").unwrap();
    assert!(acquire_destination_lock(&destination).is_err());
    fs::write(lock.join("pid"), b"999999999\n").unwrap();
    let mut guard = acquire_destination_lock(&destination).unwrap();
    guard.release(&mut ops).unwrap();
}

#[cfg(unix)]
#[test]
fn lock_protocol_refuses_symlinked_lock_directory_and_pid_file() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::TempDir::new().unwrap();
    let destination = temp.path().join("skills");
    let outside = temp.path().join("outside");
    fs::create_dir(&destination).unwrap();
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("pid"), b"123\n").unwrap();
    let lock = destination.join(".bzr-skill.lock");
    symlink(&outside, &lock).unwrap();
    let Err(error) = acquire_destination_lock(&destination) else {
        panic!("symlinked lock directory must be refused");
    };
    assert!(error.to_string().contains("symbolic link"));
    assert_eq!(fs::read(outside.join("pid")).unwrap(), b"123\n");

    fs::remove_file(&lock).unwrap();
    fs::create_dir(&lock).unwrap();
    symlink(outside.join("pid"), lock.join("pid")).unwrap();
    let Err(error) = acquire_destination_lock(&destination) else {
        panic!("symlinked lock PID must be refused");
    };
    assert!(error.to_string().contains("symbolic link"));
    assert_eq!(fs::read(outside.join("pid")).unwrap(), b"123\n");
}

#[test]
fn later_lock_contention_releases_the_first_sorted_destination_lock() {
    let temp = tempfile::TempDir::new().unwrap();
    let agents = temp.path().join(".agents/skills");
    let claude = temp.path().join(".claude/skills");
    fs::create_dir_all(&agents).unwrap();
    fs::create_dir_all(&claude).unwrap();
    let held = acquire_destination_lock(&claude).unwrap();

    let error = install_with_ops(
        request(temp.path(), AgentTarget::All),
        &mut TestOps::new(Failure::None),
    )
    .unwrap_err();

    assert!(error.to_string().contains("locked"));
    assert!(!agents.join(".bzr-skill.lock").exists());
    assert!(held.path().exists());
}

#[test]
fn lock_release_cannot_remove_a_successor_lock_generation() {
    let temp = tempfile::TempDir::new().unwrap();
    let destination = temp.path().join("skills");
    fs::create_dir(&destination).unwrap();
    let mut original = acquire_destination_lock(&destination).unwrap();
    let mut ops = TestOps::with_failures(&[Failure::ReleaseHandoff]);

    original.release(&mut ops).unwrap();

    let mut successor = ops.successor.take().unwrap();
    assert!(destination.join(".bzr-skill.lock").is_dir());
    assert!(acquire_destination_lock(&destination).is_err());
    successor.release(&mut TestOps::new(Failure::None)).unwrap();
}

#[test]
fn write_and_target_to_aside_failures_preserve_the_original_and_remove_stage() {
    for failure in [Failure::Write, Failure::TargetToAside] {
        let temp = tempfile::TempDir::new().unwrap();
        create_owned(temp.path(), SKILL);
        let before = snapshot(temp.path());
        let error = run(temp.path(), failure).unwrap_err();
        assert!(error.to_string().contains("installed before failure: none"));
        assert_eq!(snapshot(temp.path()), before);
    }
}

#[test]
fn first_install_activation_failure_leaves_no_target_and_reports_stage_cleanup() {
    let temp = tempfile::TempDir::new().unwrap();
    let error = run(temp.path(), Failure::Activate).unwrap_err();
    assert!(error.to_string().contains("activate"));
    assert!(error
        .to_string()
        .contains(&target(temp.path()).display().to_string()));
    assert!(!target(temp.path()).exists());
}

#[test]
fn activation_failure_restores_existing_owned_target() {
    let temp = tempfile::TempDir::new().unwrap();
    create_owned(temp.path(), SKILL);
    let error = run(temp.path(), Failure::Activate).unwrap_err();
    assert!(error.to_string().contains("restored previous content"));
    assert_eq!(
        fs::read(target(temp.path()).join("old.txt")).unwrap(),
        b"old bytes"
    );
}

#[test]
fn failed_restore_retains_authoritative_aside_and_stage_recovery_paths() {
    let temp = tempfile::TempDir::new().unwrap();
    create_owned(temp.path(), SKILL);
    let mut ops = TestOps::with_failures(&[Failure::Activate, Failure::Restore]);
    let error =
        install_with_ops(request(temp.path(), AgentTarget::Standard), &mut ops).unwrap_err();
    assert!(error.to_string().contains("restore failed"));
    assert!(error.to_string().contains("previous content remains"));
    assert!(!target(temp.path()).exists());
}

#[test]
fn aside_cleanup_and_lock_release_failures_are_success_warnings() {
    for failure in [Failure::RemoveAside, Failure::ReleaseLock] {
        let temp = tempfile::TempDir::new().unwrap();
        create_owned(temp.path(), SKILL);
        let outcome = run(temp.path(), failure).unwrap();
        assert_eq!(outcome.destinations[0].installed, embedded::skill_names());
        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].contains("verify"));
        assert!(target(temp.path()).join("SKILL.md").is_file());
    }
}

#[test]
fn stage_cleanup_failure_reports_the_residual_stage_path() {
    let temp = tempfile::TempDir::new().unwrap();
    let mut ops = TestOps::with_failures(&[Failure::Write, Failure::RemoveStage]);
    let error =
        install_with_ops(request(temp.path(), AgentTarget::Standard), &mut ops).unwrap_err();
    assert!(error.to_string().contains("residual stage"));
    assert!(fs::read_dir(destination(temp.path()))
        .unwrap()
        .any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains(".stage.")));
}

#[test]
fn final_recheck_preserves_foreign_bytes_introduced_after_authoritative_pass() {
    let temp = tempfile::TempDir::new().unwrap();
    let error = run(temp.path(), Failure::RaceForeign).unwrap_err();
    assert!(error.to_string().contains("foreign"));
    assert_eq!(
        fs::read(target(temp.path()).join("foreign.txt")).unwrap(),
        b"foreign bytes"
    );
}
