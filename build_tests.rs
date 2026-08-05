use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use super::{git_rerun_paths, validate_skill_name};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "bzr-build-tests-{}-{label}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create build-script test fixture");
        Self(fs::canonicalize(path).expect("canonicalize build-script test fixture"))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove build-script test fixture");
    }
}

#[test]
fn accepts_canonical_skill_names() {
    for name in ["bzr-setup", "bzr-file-bug", "skill1", "a1-b2-c3"] {
        assert!(validate_skill_name(name).is_ok(), "{name}");
    }
}

#[test]
fn rejects_non_ascii_and_control_characters() {
    for name in ["bzr-café", "bzr-file\nbug", "bzr-file\rbug"] {
        assert!(validate_skill_name(name).is_err(), "{name:?}");
    }
}

#[test]
fn rejects_empty_and_malformed_hyphen_separators() {
    for name in ["", "-bzr", "bzr-", "bzr--file", "Bzr-file", "bzr_file"] {
        assert!(validate_skill_name(name).is_err(), "{name:?}");
    }
}

#[test]
fn watches_head_and_symbolic_branch_ref_in_a_normal_checkout() {
    let fixture = Fixture::new("normal-git-dir");
    let git_dir = fixture.path().join(".git");
    fs::create_dir_all(git_dir.join("refs/heads")).expect("create refs directory");
    fs::write(git_dir.join("HEAD"), "ref: refs/heads/topic\n").expect("write HEAD");

    assert_eq!(
        git_rerun_paths(fixture.path()),
        vec![git_dir.join("HEAD"), git_dir.join("refs/heads/topic")]
    );
}

#[test]
fn watches_only_head_in_a_detached_checkout() {
    let fixture = Fixture::new("detached");
    let git_dir = fixture.path().join(".git");
    fs::create_dir(&git_dir).expect("create git directory");
    fs::write(git_dir.join("HEAD"), "0123456789abcdef\n").expect("write HEAD");

    assert_eq!(git_rerun_paths(fixture.path()), vec![git_dir.join("HEAD")]);
}

#[test]
fn resolves_worktree_head_and_shared_symbolic_ref() {
    let fixture = Fixture::new("worktree");
    let checkout = fixture.path().join("checkout");
    let common = fixture.path().join("admin");
    let git_dir = common.join("worktrees/topic");
    fs::create_dir_all(checkout.as_path()).expect("create checkout");
    fs::create_dir_all(git_dir.as_path()).expect("create worktree git directory");
    fs::create_dir_all(common.join("refs/heads")).expect("create shared refs directory");
    fs::write(checkout.join(".git"), "gitdir: ../admin/worktrees/topic\n")
        .expect("write gitdir pointer");
    fs::write(git_dir.join("commondir"), "../..\n").expect("write commondir pointer");
    fs::write(git_dir.join("HEAD"), "ref: refs/heads/topic\n").expect("write HEAD");

    assert_eq!(
        git_rerun_paths(&checkout),
        vec![git_dir.join("HEAD"), common.join("refs/heads/topic")]
    );
}

#[test]
fn package_tree_without_git_metadata_has_no_git_watch_paths() {
    let fixture = Fixture::new("package");

    assert!(git_rerun_paths(fixture.path()).is_empty());
}
