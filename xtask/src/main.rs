//! Build helpers for the `bzr` workspace.
//!
//! Currently exposes a single `man` subcommand that generates roff manpages
//! for `bzr` and all of its subcommands by introspecting the clap-derive CLI
//! tree at `bzr::cli::Cli`.
//!
//! Run via `cargo run -p xtask -- man [--out DIR]`.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask", about = "Internal build helpers for bzr")]
struct Xtask {
    #[command(subcommand)]
    cmd: XtaskCmd,
}

#[derive(Subcommand)]
enum XtaskCmd {
    /// Generate roff manpages for `bzr` and all subcommands.
    Man {
        /// Output directory; created if missing. Defaults to `man/man1` at the
        /// workspace root so packagers can consume a stable path.
        #[arg(long, default_value = "man/man1")]
        out: PathBuf,
    },
}

fn main() -> Result<()> {
    let xtask = Xtask::parse();
    match xtask.cmd {
        XtaskCmd::Man { out } => generate_man(&out),
    }
}

fn generate_man(out: &PathBuf) -> Result<()> {
    fs::create_dir_all(out)
        .with_context(|| format!("creating manpage output directory {}", out.display()))?;

    let cmd = <bzr::cli::Cli as CommandFactory>::command();

    clap_mangen::generate_to(cmd, out)
        .with_context(|| format!("writing manpages to {}", out.display()))?;

    eprintln!("wrote manpages to {}", out.display());
    Ok(())
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
