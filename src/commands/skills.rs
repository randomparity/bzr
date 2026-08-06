//! Command-layer orchestration for bundled skill installation.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::cli::{AgentTarget, InstallArgs, SkillsAction};
use crate::commands::runtime::invocation::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::resources::skills::{
    write_skills_install, SkillsDestinationResult, SkillsInstallResult,
};
use crate::output::writers::Writers;
use crate::skills::installer::{self, InstallOutcome, InstallRequest};

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
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let SkillsAction::Install(args) = action;
    execute_install_with_resolvers(
        args,
        ctx,
        w,
        installer::install,
        (std::env::current_dir, dirs::home_dir),
    )
}

#[cfg(test)]
fn execute_install_with<F>(
    args: &InstallArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
    installer_fn: F,
) -> Result<()>
where
    F: FnOnce(InstallRequest) -> Result<InstallOutcome>,
{
    execute_install_with_resolvers(
        args,
        ctx,
        w,
        installer_fn,
        (std::env::current_dir, dirs::home_dir),
    )
}

#[cfg(test)]
fn execute_install_with_current_dir<F, C>(
    args: &InstallArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
    installer_fn: F,
    current_dir: C,
) -> Result<()>
where
    F: FnOnce(InstallRequest) -> Result<InstallOutcome>,
    C: FnOnce() -> std::io::Result<PathBuf>,
{
    execute_install_with_resolvers(args, ctx, w, installer_fn, (current_dir, dirs::home_dir))
}

fn execute_install_with_resolvers<F, C, H>(
    args: &InstallArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
    installer_fn: F,
    resolvers: (C, H),
) -> Result<()>
where
    F: FnOnce(InstallRequest) -> Result<InstallOutcome>,
    C: FnOnce() -> std::io::Result<PathBuf>,
    H: FnOnce() -> Option<PathBuf>,
{
    let (current_dir, home_dir) = resolvers;
    let supplied_home = if args.project.is_none() {
        home_dir()
    } else {
        None
    };
    let resolved_home = if args.global {
        Some(resolve_global_home(supplied_home)?)
    } else {
        supplied_home.and_then(|home| resolve_global_home(Some(home)).ok())
    };
    let scope = resolve_scope_with_current_dir(
        args,
        TerminalState::current(),
        resolved_home.as_deref(),
        w.err,
        current_dir,
    )?;
    let outcome = installer_fn(InstallRequest {
        agent: args.agent,
        scope: scope.clone(),
        home: resolved_home,
    })?;
    let result = install_result(args.agent, &scope, outcome.destinations);
    write_skills_install(&result, ctx.format(), w.out);
    for warning in outcome.warnings {
        let _ = writeln!(w.err, "{warning}");
    }
    Ok(())
}

fn install_result(
    agent: AgentTarget,
    scope: &InstallScope,
    destinations: Vec<installer::InstalledDestination>,
) -> SkillsInstallResult {
    let (scope_name, project) = match scope {
        InstallScope::Global => ("global", None),
        InstallScope::Project(project) => ("project", Some(project.display().to_string())),
    };
    SkillsInstallResult {
        action: "install".into(),
        agent,
        scope: scope_name.into(),
        project,
        destinations: destinations
            .into_iter()
            .map(|destination| SkillsDestinationResult {
                layout: destination.layout.into(),
                path: destination.path.display().to_string(),
                installed: destination.installed,
            })
            .collect(),
    }
}

/// Resolve an explicit scope or write terminal-appropriate guidance for an omission.
#[cfg(test)]
fn resolve_scope(
    args: &InstallArgs,
    terminal: TerminalState,
    home: Option<&Path>,
    cwd: &Path,
    err: &mut (impl Write + ?Sized),
) -> Result<InstallScope> {
    resolve_scope_with_current_dir(args, terminal, home, err, || Ok(cwd.to_path_buf()))
}

fn resolve_scope_with_current_dir<C>(
    args: &InstallArgs,
    terminal: TerminalState,
    home: Option<&Path>,
    err: &mut (impl Write + ?Sized),
    current_dir: C,
) -> Result<InstallScope>
where
    C: FnOnce() -> std::io::Result<PathBuf>,
{
    if args.global {
        return Ok(InstallScope::Global);
    }
    if let Some(project) = &args.project {
        let project = if project.is_absolute() {
            project.clone()
        } else {
            let cwd = current_dir().map_err(|error| {
                BzrError::input(format!(
                    "could not resolve the current directory for relative skill-install project \
                     '{}': {error}",
                    project.display()
                ))
            })?;
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

    let cwd = terminal
        .is_interactive()
        .then(current_dir)
        .transpose()
        .ok()
        .flatten();
    write_scope_guidance(args.agent, terminal, home, cwd.as_deref(), err);
    Err(BzrError::input(
        "choose exactly one skill-install scope: --global or --project <PATH>".into(),
    ))
}

fn resolve_global_home(home: Option<PathBuf>) -> Result<PathBuf> {
    let home = home.ok_or_else(|| {
        BzrError::input(
            "could not resolve the home directory for global skill installation; set HOME to an \
             absolute existing directory"
                .into(),
        )
    })?;
    if !home.is_absolute() {
        return Err(BzrError::input(format!(
            "home directory '{}' for global skill installation is not absolute; set HOME to an \
             absolute existing directory",
            home.display()
        )));
    }
    let home = home.canonicalize().map_err(|error| {
        BzrError::input(format!(
            "could not resolve home directory '{}' for global skill installation: {error}",
            home.display()
        ))
    })?;
    if !home.is_dir() {
        return Err(BzrError::input(format!(
            "home directory '{}' for global skill installation is not a directory",
            home.display()
        )));
    }
    Ok(home)
}

fn write_scope_guidance(
    agent: AgentTarget,
    terminal: TerminalState,
    home: Option<&Path>,
    cwd: Option<&Path>,
    err: &mut (impl Write + ?Sized),
) {
    if terminal.is_interactive() {
        let _ = writeln!(err, "Choose where to install bzr skills:");
        write_destination_patterns("Global destinations", agent, home, err);
        write_destination_patterns("Project destinations", agent, cwd, err);
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
