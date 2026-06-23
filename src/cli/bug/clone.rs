use clap::Args;

use crate::cli::bug::CloneCreateFieldArgs;

pub const LONG_ABOUT: &str = r#"Clone an existing bug, optionally overriding fields.

Copies the source bug's product, component, version, summary,
description, priority, severity, assignee, op-sys,
rep-platform, URL, whiteboard, target milestone, deadline, CC
list, and keywords into a new bug. Pass any of the override
flags (`--summary`, `--product`, `--url`, ...) to change values
for the clone; unspecified fields inherit from the source.

By default the new bug gets a "Cloned from bug #N" comment;
disable with `--no-comment`. Use `--add-depends-on` to link
the new bug as a dependency of the source, or `--add-blocks`
to make it block the source. `--no-cc` and `--no-keywords`
skip copying those lists; `--cc` and `--keywords` replace the
copied lists. `--groups` and `--flag` are explicit additions
for the new bug and are not copied from the source.

Examples:

  bzr bug clone 12345
  bzr bug clone 12345 --summary "Backport to RHEL 9" \
    --version "RHEL 9.4" --add-depends-on

See bzr-bug-create(1) for filing a brand-new bug."#;

/// Arguments for `bug clone`.
#[derive(Args, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "clone uses independent CLI switches, not one state enum"
)]
pub struct CloneArgs {
    /// Source bug ID or alias
    pub id: String,
    /// Override summary
    #[arg(long)]
    pub summary: Option<String>,
    /// Override product
    #[arg(long)]
    pub product: Option<String>,
    /// Override component
    #[arg(long)]
    pub component: Option<String>,
    /// Override version
    #[arg(long)]
    pub version: Option<String>,
    /// Override description
    #[arg(long)]
    pub description: Option<String>,
    /// Override priority
    #[arg(long)]
    pub priority: Option<String>,
    /// Override severity
    #[arg(long)]
    pub severity: Option<String>,
    /// Override assignee
    #[arg(long)]
    pub assignee: Option<String>,
    /// Override operating system
    #[arg(long)]
    pub op_sys: Option<String>,
    /// Override hardware platform
    #[arg(long)]
    pub rep_platform: Option<String>,
    #[command(flatten)]
    pub create_fields: CloneCreateFieldArgs,
    /// Skip adding "Cloned from bug #N" comment
    #[arg(long)]
    pub no_comment: bool,
    /// Make the new bug depend on the source bug
    #[arg(long)]
    pub add_depends_on: bool,
    /// Make the new bug block the source bug
    #[arg(long)]
    pub add_blocks: bool,
    /// Don't copy the CC list from the source bug
    #[arg(long)]
    pub no_cc: bool,
    /// Don't copy keywords from the source bug
    #[arg(long)]
    pub no_keywords: bool,
}

#[cfg(test)]
#[path = "clone_tests.rs"]
mod tests;
