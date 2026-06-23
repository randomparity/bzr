//! Completion command — emits a shell completion script to stdout.
//!
//! Purely local: no config, network, or auth. The script is generated from
//! the clap `Command` tree so it always matches the binary's real surface.

use clap::CommandFactory;
use clap_complete::{generate, Shell};

use crate::cli::Cli;
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::writers::Writers;

/// Write a completion script for `shell` to stdout.
///
/// The script is generated from the live clap `Command` tree, so it stays in
/// sync with the binary's subcommands and flags without manual maintenance.
#[expect(
    clippy::unused_async,
    reason = "command handlers share the async dispatch signature"
)]
pub async fn execute(shell: Shell, _ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();
    generate(shell, &mut cmd, bin_name, &mut w.out);
    Ok(())
}

#[cfg(test)]
#[path = "completion_tests.rs"]
mod tests;
