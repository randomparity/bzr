#![expect(clippy::unwrap_used, clippy::panic)]

use std::cell::Cell;
use std::io;
use std::path::{Path, PathBuf};

use super::{
    execute_install_with, execute_install_with_current_dir, execute_install_with_resolvers,
    resolve_global_home, resolve_scope, TerminalState,
};
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
fn global_home_rejects_relative_missing_and_non_directory_paths() {
    let temp = tempfile::TempDir::new().unwrap();
    let missing = temp.path().join("missing");
    let file = temp.path().join("file");
    std::fs::write(&file, b"not a directory").unwrap();

    for home in [PathBuf::from("relative-home"), missing, file] {
        let error = resolve_global_home(Some(home)).unwrap_err();
        assert_eq!(error.exit_code(), 7);
    }
    assert_eq!(resolve_global_home(None).unwrap_err().exit_code(), 7);
}

#[test]
fn global_home_canonicalizes_changed_lexical_paths() {
    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let sibling = temp.path().join("sibling");
    std::fs::create_dir(&home).unwrap();
    std::fs::create_dir(&sibling).unwrap();

    let resolved = resolve_global_home(Some(sibling.join("..").join("home"))).unwrap();

    assert_eq!(resolved, home.canonicalize().unwrap());
}

#[cfg(unix)]
#[test]
fn global_home_accepts_a_symlink_alias_and_returns_the_canonical_directory() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let alias = temp.path().join("alias");
    std::fs::create_dir(&home).unwrap();
    symlink(&home, &alias).unwrap();

    let resolved = resolve_global_home(Some(alias)).unwrap();

    assert_eq!(resolved, home.canonicalize().unwrap());
}

#[test]
fn global_command_resolves_home_once_for_installation_and_output() {
    let temp = tempfile::TempDir::new().unwrap();
    let home = temp.path().join("home");
    let unused = temp.path().join("unused");
    std::fs::create_dir(&home).unwrap();
    std::fs::create_dir(&unused).unwrap();
    let lexical_home = unused.join("..").join("home");
    let canonical_home = home.canonicalize().unwrap();
    let calls = Cell::new(0);
    let args = InstallArgs {
        agent: AgentTarget::Codex,
        global: true,
        project: None,
    };
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut writers = Writers::new(&mut out, &mut err);
    let context =
        crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None);

    execute_install_with_resolvers(
        &args,
        &context,
        &mut writers,
        |request| {
            assert_eq!(request.home.as_deref(), Some(canonical_home.as_path()));
            Ok(InstallOutcome {
                destinations: vec![InstalledDestination {
                    layout: "agents",
                    path: request.home.unwrap().join(".agents/skills"),
                    installed: vec!["bzr-reference".into()],
                }],
                warnings: Vec::new(),
            })
        },
        (
            || Err(io::Error::other("current directory must not be queried")),
            || {
                calls.set(calls.get() + 1);
                Some(lexical_home)
            },
        ),
    )
    .unwrap();

    assert_eq!(calls.get(), 1);
    let output = String::from_utf8(out).unwrap();
    let data = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(
        data["destinations"][0]["path"],
        canonical_home.join(".agents/skills").display().to_string()
    );
    assert!(err.is_empty());
}

#[test]
fn explicit_global_command_does_not_resolve_current_directory() {
    let args = InstallArgs {
        agent: AgentTarget::Codex,
        global: true,
        project: None,
    };
    let called = Cell::new(false);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut writers = Writers::new(&mut out, &mut err);
    let context =
        crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None);

    execute_install_with_current_dir(
        &args,
        &context,
        &mut writers,
        |_| {
            Ok(InstallOutcome {
                destinations: Vec::new(),
                warnings: Vec::new(),
            })
        },
        || {
            called.set(true);
            Err(io::Error::other("injected current-dir failure"))
        },
    )
    .unwrap();

    assert!(!called.get());
}

#[test]
fn absolute_project_command_does_not_resolve_current_directory() {
    let project = tempfile::TempDir::new().unwrap();
    let args = project_args(project.path());
    let called = Cell::new(false);
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut writers = Writers::new(&mut out, &mut err);
    let context =
        crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None);

    execute_install_with_current_dir(
        &args,
        &context,
        &mut writers,
        |_| {
            Ok(InstallOutcome {
                destinations: Vec::new(),
                warnings: Vec::new(),
            })
        },
        || {
            called.set(true);
            Err(io::Error::other("injected current-dir failure"))
        },
    )
    .unwrap();

    assert!(!called.get());
}

#[test]
fn relative_project_command_reports_current_directory_failure() {
    let args = project_args(Path::new("relative-project"));
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut writers = Writers::new(&mut out, &mut err);
    let context =
        crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None);

    let error = execute_install_with_current_dir(
        &args,
        &context,
        &mut writers,
        |_| panic!("installer must not run when project resolution fails"),
        || Err(io::Error::other("injected current-dir failure")),
    )
    .unwrap_err();

    assert_eq!(error.exit_code(), 7);
    assert!(error.to_string().contains(
        "could not resolve the current directory for relative skill-install project \
         'relative-project': injected current-dir failure"
    ));
    assert!(out.is_empty());
    assert!(err.is_empty());
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

fn complete_project_table() -> &'static str {
    concat!(
        "+-----------------+-------------------------+\n",
        "| Skill           | Destination             |\n",
        "+-----------------+-------------------------+\n",
        "| bzr-bulk-triage | /project/.agents/skills |\n",
        "+-----------------+-------------------------+\n",
        "| bzr-file-bug    | /project/.agents/skills |\n",
        "+-----------------+-------------------------+\n",
        "| bzr-bulk-triage | /project/.claude/skills |\n",
        "+-----------------+-------------------------+\n",
        "| bzr-file-bug    | /project/.claude/skills |\n",
        "+-----------------+-------------------------+\n",
    )
}

fn assert_install_error_is_silent(message: &str) {
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
        |_| Err(BzrError::DataIntegrity(message.into())),
    );

    let Err(error @ BzrError::DataIntegrity(_)) = result else {
        panic!("installer failure must retain its classification");
    };
    assert_eq!(error.error_type(), "data_integrity");
    let BzrError::DataIntegrity(actual) = error else {
        unreachable!("variant was checked above");
    };
    assert_eq!(actual, message);
    assert!(out.is_empty());
    assert!(err.is_empty());
}

#[test]
fn partial_activation_error_retains_classification_and_emits_nothing() {
    assert_install_error_is_silent(
        "activate '/project/bzr-file-bug' failed; installed before failure: \
         bzr-bulk-triage -> /project/.agents/skills",
    );
}

#[test]
fn failed_restore_error_retains_recovery_paths_and_emits_nothing() {
    assert_install_error_is_silent(
        "activate '/project/bzr-bulk-triage' failed; restore failed: injected Restore; \
         previous content remains at '/project/.bzr-skill.old'; staged content remains at \
         '/project/.bzr-skill.stage'",
    );
}

#[test]
fn lock_cleanup_warning_emits_complete_json_and_exact_recovery_stderr() {
    let project = tempfile::TempDir::new().unwrap();
    let canonical = project.path().canonicalize().unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut writers = Writers::new(&mut out, &mut err);
    let context =
        crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None);
    let warning = "installation completed but could not release lock \
                   '/project/.bzr-skill.lock.release.42': injected ReleaseLock; verify no bzr \
                   skills install process is using it before removing it";

    execute_install_with(
        &project_args(project.path()),
        &context,
        &mut writers,
        |_| Ok(successful_outcome(&canonical, vec![warning.into()])),
    )
    .unwrap();

    let output = String::from_utf8(out).unwrap();
    let data = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(
        data,
        serde_json::json!({
            "action": "install",
            "agent": "all",
            "scope": "project",
            "project": canonical.display().to_string(),
            "destinations": [
                {
                    "layout": "agents",
                    "path": canonical.join(".agents/skills").display().to_string(),
                    "installed": ["bzr-bulk-triage", "bzr-file-bug"]
                },
                {
                    "layout": "claude",
                    "path": canonical.join(".claude/skills").display().to_string(),
                    "installed": ["bzr-bulk-triage", "bzr-file-bug"]
                }
            ]
        })
    );
    assert_eq!(String::from_utf8(err).unwrap(), format!("{warning}\n"));
}

#[test]
fn aside_cleanup_warning_emits_complete_table_and_exact_recovery_stderr() {
    let project = tempfile::TempDir::new().unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut writers = Writers::new(&mut out, &mut err);
    let context =
        crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None);

    let warning = "installed 'bzr-bulk-triage' at '/project/.agents/skills/bzr-bulk-triage' \
                   but could not remove residual aside '/project/.agents/skills/.bzr-skill.old': \
                   injected RemoveAside; verify the installed target, then remove the aside";

    execute_install_with(
        &project_args(project.path()),
        &context,
        &mut writers,
        |_| {
            Ok(successful_outcome(
                Path::new("/project"),
                vec![warning.into()],
            ))
        },
    )
    .unwrap();

    let output = String::from_utf8(out).unwrap();
    assert_eq!(output, complete_project_table());
    assert_eq!(String::from_utf8(err).unwrap(), format!("{warning}\n"));
}
