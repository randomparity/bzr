#![expect(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use super::{resolve_scope, TerminalState};
use crate::cli::{AgentTarget, InstallArgs};
use crate::error::BzrError;

fn missing_scope_args() -> InstallArgs {
    InstallArgs {
        agent: AgentTarget::All,
        global: false,
        project: None,
    }
}

#[test]
fn omitted_scope_at_interactive_terminal_shows_resolved_choices_and_examples() {
    let mut err = Vec::new();
    let result = resolve_scope(
        &missing_scope_args(),
        TerminalState::new(true, true),
        Some(Path::new("/Users/alice")),
        Path::new("/work/project"),
        &mut err,
    );

    let Err(error) = result else {
        panic!("missing scope must fail execution");
    };
    assert_eq!(error.exit_code(), 7);
    let error = String::from_utf8(err).unwrap();
    assert!(error.contains("/Users/alice/.agents/skills"));
    assert!(error.contains("/Users/alice/.claude/skills"));
    assert!(error.contains("/work/project/.agents/skills"));
    assert!(error.contains("/work/project/.claude/skills"));
    assert!(error.contains("bzr skills install --agent all --global"));
    assert!(error.contains("bzr skills install --agent all --project ."));
}

#[test]
fn omitted_scope_without_interactive_terminals_prints_concise_examples_only() {
    let mut err = Vec::new();
    let result = resolve_scope(
        &missing_scope_args(),
        TerminalState::new(false, false),
        Some(Path::new("/Users/alice")),
        Path::new("/work/project"),
        &mut err,
    );

    let Err(BzrError::InputValidation { .. }) = result else {
        panic!("missing scope must be an input-validation error");
    };
    let error = String::from_utf8(err).unwrap();
    assert!(error.contains("bzr skills install --agent all --global"));
    assert!(error.contains("bzr skills install --agent all --project ."));
    assert!(!error.contains("/Users/alice/.agents/skills"));
    assert!(!error.contains("/work/project/.agents/skills"));
}

#[test]
fn explicit_global_resolves_without_reading_filesystem() {
    let args = InstallArgs {
        agent: AgentTarget::Codex,
        global: true,
        project: None,
    };
    let scope = resolve_scope(
        &args,
        TerminalState::new(false, false),
        None,
        Path::new("/work/project"),
        &mut Vec::new(),
    )
    .unwrap();

    assert_eq!(format!("{scope:?}"), "Global");
}

#[test]
fn explicit_project_resolves_to_a_canonical_absolute_root() {
    let project = tempfile::TempDir::new().unwrap();
    let args = InstallArgs {
        agent: AgentTarget::Claude,
        global: false,
        project: Some(PathBuf::from(".")),
    };
    let scope = resolve_scope(
        &args,
        TerminalState::new(false, false),
        None,
        project.path(),
        &mut Vec::new(),
    )
    .unwrap();

    assert_eq!(
        scope,
        super::InstallScope::Project(project.path().canonicalize().unwrap())
    );
}
