#![expect(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use super::{execute_install_with, resolve_scope, TerminalState};
use crate::cli::{AgentTarget, InstallArgs};
use crate::error::BzrError;
use crate::output::writers::Writers;
use crate::skills::installer::{InstallOutcome, InstalledDestination};
use crate::types::OutputFormat;

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

#[test]
fn project_scope_rejects_nonexistent_and_non_directory_roots() {
    let temp = tempfile::TempDir::new().unwrap();
    let file = temp.path().join("file");
    std::fs::write(&file, b"not a project directory").unwrap();
    for project in [temp.path().join("missing"), file] {
        let args = InstallArgs {
            agent: AgentTarget::Standard,
            global: false,
            project: Some(project),
        };
        let error = resolve_scope(
            &args,
            TerminalState::new(false, false),
            None,
            temp.path(),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert_eq!(error.exit_code(), 7);
    }
}

#[cfg(unix)]
#[test]
fn project_scope_accepts_a_root_symlink_alias_but_returns_the_canonical_root() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::TempDir::new().unwrap();
    let project = temp.path().join("project");
    let alias = temp.path().join("alias");
    std::fs::create_dir(&project).unwrap();
    symlink(&project, &alias).unwrap();
    let args = InstallArgs {
        agent: AgentTarget::Claude,
        global: false,
        project: Some(alias),
    };

    let scope = resolve_scope(
        &args,
        TerminalState::new(false, false),
        None,
        temp.path(),
        &mut Vec::new(),
    )
    .unwrap();

    assert_eq!(
        scope,
        super::InstallScope::Project(project.canonicalize().unwrap())
    );
}

fn project_args(project: &Path) -> InstallArgs {
    InstallArgs {
        agent: AgentTarget::All,
        global: false,
        project: Some(project.to_path_buf()),
    }
}

fn successful_outcome(project: &Path, warnings: Vec<String>) -> InstallOutcome {
    InstallOutcome {
        destinations: vec![
            InstalledDestination {
                layout: "agents",
                path: project.join(".agents/skills"),
                installed: vec!["bzr-bulk-triage".into(), "bzr-file-bug".into()],
            },
            InstalledDestination {
                layout: "claude",
                path: project.join(".claude/skills"),
                installed: vec!["bzr-bulk-triage".into(), "bzr-file-bug".into()],
            },
        ],
        warnings,
    }
}

#[test]
fn installer_activation_or_restore_error_returns_classified_error_with_empty_stdout() {
    let project = tempfile::TempDir::new().unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut writers = Writers::new(&mut out, &mut err);
    let context =
        crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None);

    let result = execute_install_with(
        &project_args(project.path()),
        &context,
        &mut writers,
        |_| {
            Err(BzrError::DataIntegrity(
                "activate '/project/skill' failed; installed before failure: bzr-one -> /project"
                    .into(),
            ))
        },
    );

    let Err(BzrError::DataIntegrity(message)) = result else {
        panic!("installer failure must retain its classification");
    };
    assert!(message.contains("installed before failure"));
    assert!(out.is_empty());
    assert!(err.is_empty());
}

#[test]
fn successful_cleanup_warning_emits_complete_json_and_recovery_stderr() {
    let project = tempfile::TempDir::new().unwrap();
    let canonical = project.path().canonicalize().unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut writers = Writers::new(&mut out, &mut err);
    let context =
        crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None);
    let warning = "installation completed but could not release lock '/project/.bzr-skill.lock'; verify no process is using it";

    execute_install_with(
        &project_args(project.path()),
        &context,
        &mut writers,
        |_| Ok(successful_outcome(&canonical, vec![warning.into()])),
    )
    .unwrap();

    let output = String::from_utf8(out).unwrap();
    let data = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(data["action"], "install");
    assert_eq!(data["agent"], "all");
    assert_eq!(data["scope"], "project");
    assert_eq!(data["project"], canonical.display().to_string());
    assert_eq!(data["destinations"][0]["layout"], "agents");
    assert_eq!(data["destinations"][1]["layout"], "claude");
    assert_eq!(
        data["destinations"][0]["installed"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(String::from_utf8(err).unwrap(), format!("{warning}\n"));
}

#[test]
fn successful_install_emits_the_complete_table_through_the_same_seam() {
    let project = tempfile::TempDir::new().unwrap();
    let canonical = project.path().canonicalize().unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut writers = Writers::new(&mut out, &mut err);
    let context =
        crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None);

    execute_install_with(
        &project_args(project.path()),
        &context,
        &mut writers,
        |_| Ok(successful_outcome(&canonical, Vec::new())),
    )
    .unwrap();

    let output = String::from_utf8(out).unwrap();
    assert!(output.contains("bzr-bulk-triage"));
    assert!(output.contains(&canonical.join(".agents/skills").display().to_string()));
    assert!(output.contains(&canonical.join(".claude/skills").display().to_string()));
    assert!(err.is_empty());
}
