#![expect(clippy::panic)]

use clap::Parser as _;

use super::Cli;

#[test]
fn skills_install_accepts_each_agent_target() {
    for agent in ["standard", "bob", "codex", "claude", "all"] {
        let parsed =
            Cli::try_parse_from(["bzr", "skills", "install", "--agent", agent, "--global"]);
        assert!(parsed.is_ok(), "{agent} should be an accepted agent target");
    }
}

#[test]
fn skills_install_accepts_global_scope() {
    let parsed = Cli::try_parse_from(["bzr", "skills", "install", "--agent", "codex", "--global"]);
    assert!(parsed.is_ok());
}

#[test]
fn skills_install_accepts_project_scope() {
    let parsed = Cli::try_parse_from([
        "bzr",
        "skills",
        "install",
        "--agent",
        "claude",
        "--project",
        "workspace",
    ]);
    assert!(parsed.is_ok());
}

#[test]
fn skills_install_accepts_dot_project_scope() {
    let parsed = Cli::try_parse_from([
        "bzr",
        "skills",
        "install",
        "--agent",
        "all",
        "--project",
        ".",
    ]);
    assert!(parsed.is_ok());
}

#[test]
fn skills_install_allows_missing_scope_to_reach_execution_guidance() {
    let parsed = Cli::try_parse_from(["bzr", "skills", "install", "--agent", "standard"]);
    assert!(parsed.is_ok());
}

#[test]
fn skills_install_rejects_global_and_project_together() {
    let parsed = Cli::try_parse_from([
        "bzr",
        "skills",
        "install",
        "--agent",
        "standard",
        "--global",
        "--project",
        ".",
    ]);
    let Err(error) = parsed else {
        panic!("conflicting scopes must be rejected by clap");
    };
    assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
}
