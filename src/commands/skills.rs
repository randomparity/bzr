//! Command-layer orchestration for bundled skill installation.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::cli::{AgentTarget, InstallArgs, SkillsAction};
use crate::commands::runtime::invocation::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::writers::Writers;

/// The explicitly selected installation root after command-time resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InstallScope {
    Global,
    Project(PathBuf),
}

/// Terminal state consulted only for scope guidance.
#[derive(Debug, Clone, Copy)]
struct TerminalState {
    stdin_is_terminal: bool,
    stderr_is_terminal: bool,
}

impl TerminalState {
    #[cfg(test)]
    const fn new(stdin_is_terminal: bool, stderr_is_terminal: bool) -> Self {
        Self {
            stdin_is_terminal,
            stderr_is_terminal,
        }
    }

    fn current() -> Self {
        Self {
            stdin_is_terminal: std::io::stdin().is_terminal(),
            stderr_is_terminal: std::io::stderr().is_terminal(),
        }
    }

    const fn is_interactive(self) -> bool {
        self.stdin_is_terminal && self.stderr_is_terminal
    }
}

#[expect(
    clippy::unused_async,
    reason = "command handlers share the async dispatch signature"
)]
pub(crate) async fn execute(
    action: &SkillsAction,
    _ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let SkillsAction::Install(args) = action;
    let cwd = std::env::current_dir().map_err(|error| {
        BzrError::input(format!(
            "could not resolve the current directory for skill installation: {error}"
        ))
    })?;
    let scope = resolve_scope(
        args,
        TerminalState::current(),
        dirs::home_dir().as_deref(),
        &cwd,
        w.err,
    )?;

    let scope_name = match scope {
        InstallScope::Global => "global",
        InstallScope::Project(_) => "project",
    };
    Err(BzrError::input(format!(
        "skill installation for the {scope_name} scope is not available yet"
    )))
}

/// Resolve an explicit scope or write terminal-appropriate guidance for an omission.
fn resolve_scope(
    args: &InstallArgs,
    terminal: TerminalState,
    home: Option<&Path>,
    cwd: &Path,
    err: &mut (impl Write + ?Sized),
) -> Result<InstallScope> {
    if args.global {
        return Ok(InstallScope::Global);
    }
    if let Some(project) = &args.project {
        let project = if project.is_absolute() {
            project.clone()
        } else {
            cwd.join(project)
        };
        let project = project.canonicalize().map_err(|error| {
            BzrError::input(format!(
                "could not resolve skill-install project '{}': {error}",
                project.display()
            ))
        })?;
        if !project.is_dir() {
            return Err(BzrError::input(format!(
                "skill-install project '{}' is not a directory",
                project.display()
            )));
        }
        return Ok(InstallScope::Project(project));
    }

    write_scope_guidance(args.agent, terminal, home, cwd, err);
    Err(BzrError::input(
        "choose exactly one skill-install scope: --global or --project <PATH>".into(),
    ))
}

fn write_scope_guidance(
    agent: AgentTarget,
    terminal: TerminalState,
    home: Option<&Path>,
    cwd: &Path,
    err: &mut (impl Write + ?Sized),
) {
    if terminal.is_interactive() {
        let _ = writeln!(err, "Choose where to install bzr skills:");
        write_destination_patterns("Global destinations", agent, home, err);
        write_destination_patterns("Project destinations", agent, Some(cwd), err);
    }
    let agent = agent_name(agent);
    let _ = writeln!(err, "Examples:");
    let _ = writeln!(err, "  bzr skills install --agent {agent} --global");
    let _ = writeln!(err, "  bzr skills install --agent {agent} --project .");
}

fn write_destination_patterns(
    label: &str,
    agent: AgentTarget,
    root: Option<&Path>,
    err: &mut (impl Write + ?Sized),
) {
    let Some(root) = root else {
        let _ = writeln!(
            err,
            "  {label}: unavailable (home directory could not be resolved)"
        );
        return;
    };
    if installs_agents_layout(agent) {
        let _ = writeln!(err, "  {label}: {}", root.join(".agents/skills").display());
    }
    if installs_claude_layout(agent) {
        let _ = writeln!(err, "  {label}: {}", root.join(".claude/skills").display());
    }
}

const fn agent_name(agent: AgentTarget) -> &'static str {
    match agent {
        AgentTarget::Standard => "standard",
        AgentTarget::Bob => "bob",
        AgentTarget::Codex => "codex",
        AgentTarget::Claude => "claude",
        AgentTarget::All => "all",
    }
}

const fn installs_agents_layout(agent: AgentTarget) -> bool {
    match agent {
        AgentTarget::Standard | AgentTarget::Bob | AgentTarget::Codex | AgentTarget::All => true,
        AgentTarget::Claude => false,
    }
}

const fn installs_claude_layout(agent: AgentTarget) -> bool {
    match agent {
        AgentTarget::Claude | AgentTarget::All => true,
        AgentTarget::Standard | AgentTarget::Bob | AgentTarget::Codex => false,
    }
}

#[cfg(test)]
#[path = "skills_tests.rs"]
mod tests;
