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
    FinalCheck,
    ReleaseLock,
    ReleaseHandoff,
    RaceForeign,
    RaceForeignAtAside,
    DestinationCreateRace,
    DestinationCreateRaceFile,
    DestinationCreateRaceSymlink,
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
    fn create_dir(&mut self, path: &Path) -> std::io::Result<()> {
        let race = [
            Failure::DestinationCreateRace,
            Failure::DestinationCreateRaceFile,
            Failure::DestinationCreateRaceSymlink,
        ]
        .into_iter()
        .find(|failure| self.failures.contains(failure) && self.triggered.insert(*failure));
        if let Some(race) = race {
            match race {
                Failure::DestinationCreateRace => fs::create_dir(path)?,
                Failure::DestinationCreateRaceFile => fs::write(path, b"foreign bytes")?,
                #[cfg(unix)]
                Failure::DestinationCreateRaceSymlink => {
                    std::os::unix::fs::symlink(path.parent().unwrap(), path)?;
                }
                #[cfg(not(unix))]
                Failure::DestinationCreateRaceSymlink => unreachable!(),
                _ => unreachable!(),
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "injected concurrent directory creation",
            ));
        }
        fs::create_dir(path)
    }

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
            if self.failures.contains(&Failure::RaceForeignAtAside)
                && self.triggered.insert(Failure::RaceForeignAtAside)
                && from.ends_with(SKILL)
            {
                fs::remove_dir_all(from)?;
                fs::create_dir(from)?;
                fs::write(from.join("foreign.txt"), b"foreign bytes")?;
            }
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
        self.fail_once(Failure::FinalCheck)?;
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
        concat!(
            "managed-by: bzr-skill\n",
            "installed-skill: {}\n",
            "source-version: test\n",
            "source-commit: abc\n"
        ),
        skill
    )
}

fn installed_sentinel(skill: &str) -> String {
    format!(
        "managed-by: bzr-skill\ninstalled-skill: {skill}\nsource-version: {}\nsource-commit: {}\n",
        env!("CARGO_PKG_VERSION"),
        env!("BZR_GIT_SHA")
    )
}

fn embedded_file_bytes(skill: &str, relative: &str) -> &'static [u8] {
    let path = format!("{skill}/{relative}");
    embedded::files()
        .iter()
        .find(|file| file.relative_path == path)
        .unwrap()
        .bytes
}

fn assert_installed_skill_bytes(root: &Path, skill: &str) {
    let installed = destination(root).join(skill);
    assert_eq!(
        fs::read(installed.join("SKILL.md")).unwrap(),
        embedded_file_bytes(skill, "SKILL.md")
    );
    assert_eq!(
        fs::read(installed.join(".bzr-skill-managed")).unwrap(),
        installed_sentinel(skill).as_bytes()
    );
}

fn residual_paths(destination: &Path, prefix: &str) -> Vec<PathBuf> {
    let mut paths = fs::read_dir(destination)
        .unwrap()
        .collect::<std::io::Result<Vec<_>>>()
        .unwrap()
        .into_iter()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(prefix))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn only_residual_path(destination: &Path, prefix: &str) -> PathBuf {
    let paths = residual_paths(destination, prefix);
    assert_eq!(paths.len(), 1, "expected one {prefix} residual: {paths:?}");
    paths.into_iter().next().unwrap()
}

fn expected_skill_snapshot(skill: &str) -> BTreeMap<PathBuf, (String, Vec<u8>)> {
    let mut expected = BTreeMap::new();
    let prefix = format!("{skill}/");
    for file in embedded::files() {
        let Some(relative) = file.relative_path.strip_prefix(&prefix) else {
            continue;
        };
        let relative = PathBuf::from(relative);
        let mut directory = PathBuf::new();
        if let Some(parent) = relative.parent() {
            for component in parent.components() {
                directory.push(component);
                expected.insert(directory.clone(), ("dir".into(), Vec::new()));
            }
        }
        expected.insert(relative, ("file".into(), file.bytes.to_vec()));
    }
    expected.insert(
        PathBuf::from(".bzr-skill-managed"),
        ("file".into(), installed_sentinel(skill).into_bytes()),
    );
    expected
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
fn destination_creation_race_reinspects_the_concurrently_created_directory() {
    let temp = tempfile::TempDir::new().unwrap();
    let mut ops = TestOps::new(Failure::DestinationCreateRace);

    let outcome = install_with_ops(request(temp.path(), AgentTarget::Standard), &mut ops).unwrap();

    assert!(ops.triggered.contains(&Failure::DestinationCreateRace));
    assert_eq!(outcome.destinations[0].installed, embedded::skill_names());
    assert!(destination(temp.path()).is_dir());
}

#[test]
fn destination_creation_race_rejects_and_preserves_a_concurrently_created_file() {
    let temp = tempfile::TempDir::new().unwrap();
    let mut ops = TestOps::new(Failure::DestinationCreateRaceFile);

    let error =
        install_with_ops(request(temp.path(), AgentTarget::Standard), &mut ops).unwrap_err();

    assert!(ops.triggered.contains(&Failure::DestinationCreateRaceFile));
    assert!(error.to_string().contains("not a directory"));
    assert_eq!(
        fs::read(temp.path().join(".agents")).unwrap(),
        b"foreign bytes"
    );
}

#[cfg(unix)]
#[test]
fn destination_creation_race_rejects_and_preserves_a_concurrently_created_symlink() {
    let temp = tempfile::TempDir::new().unwrap();
    let mut ops = TestOps::new(Failure::DestinationCreateRaceSymlink);

    let error =
        install_with_ops(request(temp.path(), AgentTarget::Standard), &mut ops).unwrap_err();

    assert!(ops
        .triggered
        .contains(&Failure::DestinationCreateRaceSymlink));
    assert!(error.to_string().contains("symbolic link"));
    assert!(fs::symlink_metadata(temp.path().join(".agents"))
        .unwrap()
        .file_type()
        .is_symlink());
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
            concat!(
                "\u{feff}managed-by: bzr-skill\r\n",
                "installed-skill: {}\r\n",
                "source-version: test\r\n",
                "source-commit: abc\r\n",
                "future-field: accepted\r\n"
            ),
            SKILL
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
            Some(concat!(
                "managed-by: other\n",
                "installed-skill: bzr-bulk-triage\n",
                "source-version: x\n",
                "source-commit: y\n"
            )),
        ),
        (
            "duplicate-manager",
            Some(concat!(
                "managed-by: bzr-skill\n",
                "managed-by: bzr-skill\n",
                "installed-skill: bzr-bulk-triage\n",
                "source-version: x\n",
                "source-commit: y\n"
            )),
        ),
        (
            "missing-field",
            Some("managed-by: bzr-skill\ninstalled-skill: bzr-bulk-triage\nsource-version: x\n"),
        ),
        (
            "empty-field",
            Some(concat!(
                "managed-by: bzr-skill\n",
                "installed-skill: bzr-bulk-triage\n",
                "source-version: \n",
                "source-commit: y\n"
            )),
        ),
        (
            "wrong-skill",
            Some(concat!(
                "managed-by: bzr-skill\n",
                "installed-skill: other\n",
                "source-version: x\n",
                "source-commit: y\n"
            )),
        ),
        (
            "malformed",
            Some(concat!(
                "managed-by:bzr-skill\n",
                "installed-skill: bzr-bulk-triage\n",
                "source-version: x\n",
                "source-commit: y\n"
            )),
        ),
        (
            "extra-trailing-newline",
            Some(concat!(
                "managed-by: bzr-skill\n",
                "installed-skill: bzr-bulk-triage\n",
                "source-version: x\n",
                "source-commit: y\n\n"
            )),
        ),
        (
            "bare-trailing-carriage-return",
            Some(concat!(
                "managed-by: bzr-skill\n",
                "installed-skill: bzr-bulk-triage\n",
                "source-version: x\n",
                "source-commit: y\r"
            )),
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

#[test]
fn stale_lock_with_extra_file_or_directory_is_preserved() {
    for extra_name in ["foreign-file", "foreign-directory"] {
        let temp = tempfile::TempDir::new().unwrap();
        let destination = temp.path().join("skills");
        let lock = destination.join(".bzr-skill.lock");
        fs::create_dir_all(&lock).unwrap();
        fs::write(lock.join("pid"), b"999999999\n").unwrap();
        let extra = lock.join(extra_name);
        if extra_name == "foreign-file" {
            fs::write(&extra, b"foreign bytes").unwrap();
        } else {
            fs::create_dir(&extra).unwrap();
            fs::write(extra.join("nested"), b"nested foreign bytes").unwrap();
        }

        let Err(error) = acquire_destination_lock(&destination) else {
            panic!("lock with extra entry must be refused");
        };

        assert!(error.to_string().contains("unexpected entry"));
        assert_eq!(fs::read(lock.join("pid")).unwrap(), b"999999999\n");
        if extra_name == "foreign-file" {
            assert_eq!(fs::read(extra).unwrap(), b"foreign bytes");
        } else {
            assert_eq!(
                fs::read(extra.join("nested")).unwrap(),
                b"nested foreign bytes"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn stale_lock_with_extra_symlink_is_preserved() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::TempDir::new().unwrap();
    let destination = temp.path().join("skills");
    let lock = destination.join(".bzr-skill.lock");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&lock).unwrap();
    fs::write(lock.join("pid"), b"999999999\n").unwrap();
    fs::write(&outside, b"outside bytes").unwrap();
    symlink(&outside, lock.join("foreign-link")).unwrap();

    let Err(error) = acquire_destination_lock(&destination) else {
        panic!("lock with extra symlink must be refused");
    };

    assert!(error.to_string().contains("unexpected entry"));
    assert_eq!(fs::read(lock.join("pid")).unwrap(), b"999999999\n");
    assert_eq!(fs::read(&outside).unwrap(), b"outside bytes");
    assert_eq!(fs::read_link(lock.join("foreign-link")).unwrap(), outside);
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
    assert!(residual_paths(&destination(temp.path()), ".bzr-skill.stage.").is_empty());
    assert!(residual_paths(&destination(temp.path()), ".bzr-skill.old.").is_empty());
}

#[test]
fn activation_failure_restores_existing_owned_target() {
    let temp = tempfile::TempDir::new().unwrap();
    create_owned(temp.path(), SKILL);
    let before = snapshot(temp.path());
    let error = run(temp.path(), Failure::Activate).unwrap_err();

    assert!(error.to_string().contains("restored previous content"));
    assert_eq!(snapshot(temp.path()), before);
    assert!(residual_paths(&destination(temp.path()), ".bzr-skill.stage.").is_empty());
    assert!(residual_paths(&destination(temp.path()), ".bzr-skill.old.").is_empty());
}

#[test]
fn failed_restore_retains_authoritative_aside_and_stage_recovery_paths() {
    let temp = tempfile::TempDir::new().unwrap();
    create_owned(temp.path(), SKILL);
    let mut ops = TestOps::with_failures(&[Failure::Activate, Failure::Restore]);
    let error =
        install_with_ops(request(temp.path(), AgentTarget::Standard), &mut ops).unwrap_err();
    let message = error.to_string();
    let aside = only_residual_path(&destination(temp.path()), ".bzr-skill.old.");
    let stage = only_residual_path(&destination(temp.path()), ".bzr-skill.stage.");

    assert!(message.contains("restore failed"));
    assert!(message.contains(&aside.display().to_string()));
    assert!(message.contains(&stage.display().to_string()));
    assert!(!target(temp.path()).exists());
    assert_eq!(fs::read(aside.join("old.txt")).unwrap(), b"old bytes");
    assert_eq!(
        fs::read(aside.join(".bzr-skill-managed")).unwrap(),
        sentinel(SKILL).as_bytes()
    );
    assert_eq!(
        fs::read(stage.join("SKILL.md")).unwrap(),
        embedded_file_bytes(SKILL, "SKILL.md")
    );
    assert_eq!(
        fs::read(stage.join(".bzr-skill-managed")).unwrap(),
        installed_sentinel(SKILL).as_bytes()
    );
}

#[test]
fn aside_cleanup_failure_keeps_new_target_and_old_aside_bytes() {
    let temp = tempfile::TempDir::new().unwrap();
    create_owned(temp.path(), SKILL);
    let outcome = run(temp.path(), Failure::RemoveAside).unwrap();
    let aside = only_residual_path(&destination(temp.path()), ".bzr-skill.old.");

    assert_eq!(outcome.destinations[0].installed, embedded::skill_names());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains(&aside.display().to_string()));
    assert_installed_skill_bytes(temp.path(), SKILL);
    assert_eq!(fs::read(aside.join("old.txt")).unwrap(), b"old bytes");
    assert_eq!(
        fs::read(aside.join(".bzr-skill-managed")).unwrap(),
        sentinel(SKILL).as_bytes()
    );
}

#[test]
fn lock_release_failure_keeps_installed_target_and_owned_detached_lock() {
    let temp = tempfile::TempDir::new().unwrap();
    let outcome = run(temp.path(), Failure::ReleaseLock).unwrap();
    let detached = only_residual_path(&destination(temp.path()), ".bzr-skill.lock.release.");

    assert_eq!(outcome.destinations[0].installed, embedded::skill_names());
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains(&detached.display().to_string()));
    for skill in embedded::skill_names() {
        assert_installed_skill_bytes(temp.path(), skill);
    }
    assert!(!destination(temp.path()).join(".bzr-skill.lock").exists());
    assert_eq!(
        fs::read(detached.join("pid")).unwrap(),
        format!("{}\n", std::process::id()).as_bytes()
    );
    let mut successor = acquire_destination_lock(&destination(temp.path())).unwrap();
    successor.release(&mut TestOps::new(Failure::None)).unwrap();
    assert!(detached.is_dir());
}

#[test]
fn stage_cleanup_failure_reports_the_residual_stage_path() {
    let temp = tempfile::TempDir::new().unwrap();
    let mut ops = TestOps::with_failures(&[Failure::FinalCheck, Failure::RemoveStage]);
    let error =
        install_with_ops(request(temp.path(), AgentTarget::Standard), &mut ops).unwrap_err();
    let stage = only_residual_path(&destination(temp.path()), ".bzr-skill.stage.");
    let expected_detail = format!(
        "could not remove residual stage '{}': injected RemoveStage",
        stage.display()
    );

    assert!(error.to_string().contains(&expected_detail));
    assert!(!target(temp.path()).exists());
    assert!(residual_paths(&destination(temp.path()), ".bzr-skill.old.").is_empty());
    assert_eq!(snapshot(&stage), expected_skill_snapshot(SKILL));
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

#[test]
fn post_check_foreign_swap_is_restored_without_activation_or_residuals() {
    let temp = tempfile::TempDir::new().unwrap();
    create_owned(temp.path(), SKILL);

    let error = run(temp.path(), Failure::RaceForeignAtAside).unwrap_err();

    assert!(error.to_string().contains("foreign"));
    assert!(error.to_string().contains("restored foreign content"));
    assert_eq!(
        fs::read(target(temp.path()).join("foreign.txt")).unwrap(),
        b"foreign bytes"
    );
    assert!(!target(temp.path()).join("SKILL.md").exists());
    assert!(residual_paths(&destination(temp.path()), ".bzr-skill.stage.").is_empty());
    assert!(residual_paths(&destination(temp.path()), ".bzr-skill.old.").is_empty());
}

#[test]
fn failed_post_check_foreign_restore_retains_both_recovery_paths_and_bytes() {
    let temp = tempfile::TempDir::new().unwrap();
    create_owned(temp.path(), SKILL);
    let mut ops = TestOps::with_failures(&[Failure::RaceForeignAtAside, Failure::Restore]);

    let error =
        install_with_ops(request(temp.path(), AgentTarget::Standard), &mut ops).unwrap_err();
    let message = error.to_string();
    let aside = only_residual_path(&destination(temp.path()), ".bzr-skill.old.");
    let stage = only_residual_path(&destination(temp.path()), ".bzr-skill.stage.");

    assert!(message.contains("foreign"));
    assert!(message.contains("restore failed"));
    assert!(message.contains(&aside.display().to_string()));
    assert!(message.contains(&stage.display().to_string()));
    assert!(!target(temp.path()).exists());
    assert_eq!(
        fs::read(aside.join("foreign.txt")).unwrap(),
        b"foreign bytes"
    );
    assert_eq!(snapshot(&stage), expected_skill_snapshot(SKILL));
}
