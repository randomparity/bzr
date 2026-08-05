//! Command-line arguments for installing bzr's bundled agent skills.

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

/// A supported agent layout selected by the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
#[value(rename_all = "lower")]
pub(crate) enum AgentTarget {
    Standard,
    Bob,
    Codex,
    Claude,
    All,
}

/// Arguments for `skills install`.
#[derive(Debug, Args)]
pub(crate) struct InstallArgs {
    /// Agent layout to receive every bundled bzr skill.
    #[arg(long, value_enum)]
    pub agent: AgentTarget,

    /// Install into the selected agent's global skill directory.
    #[arg(long, conflicts_with = "project")]
    pub global: bool,

    /// Install below this existing project directory.
    #[arg(long, value_name = "PATH", conflicts_with = "global")]
    pub project: Option<PathBuf>,
}

/// Manage bzr's bundled coding-agent skills.
#[derive(Debug, Subcommand)]
pub(crate) enum SkillsAction {
    /// Install every bundled bzr skill for one agent layout.
    ///
    /// Select the agent with `--agent` and one scope with `--global` or
    /// `--project <PATH>`. `--project .` uses the current directory. Omitting
    /// scope is accepted by the parser so execution can offer terminal-aware,
    /// copyable guidance without reading stdin.
    ///
    /// Examples:
    ///
    ///   bzr skills install --agent codex --global
    ///   bzr skills install --agent all --project .
    ///
    /// The install payload is embedded in the running binary, so this command
    /// works offline and never contacts a Bugzilla server.
    #[command(verbatim_doc_comment)]
    Install(InstallArgs),
}
