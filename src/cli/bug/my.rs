use clap::Args;

use crate::cli::bug::{BugFilterArgs, FieldArgs, PageArgs, SortArgs};

pub const LONG_ABOUT: &str = r"Show bugs related to the authenticated user.

Default view: bugs assigned to the caller. `--created`
switches to bugs the caller filed; `--cc` switches to bugs
the caller is CC'd on; `--all` shows all three categories
at once (assigned, created, CC'd) and conflicts with the
other two flags.

`--limit` is per category, not total -- with `--all` and
`--limit 50`, up to 150 rows may be returned. `--status`
filters across whichever category is active, with the same
repeatability and `!`-prefix semantics as `bzr bug list`.

Examples:

  bzr bug my
  bzr bug my --created --status NEW
  bzr bug my --all --limit 25

See bzr-bug-list(1) for filter-driven listing without the
caller-relative shortcuts and bzr-whoami(1) to confirm which
account `my` resolves to.";

/// Arguments for `bug my`.
#[derive(Args, Debug, Default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "my uses independent CLI view switches, not one state enum"
)]
pub(crate) struct MyArgs {
    /// Show bugs I created (instead of assigned to me).
    ///
    /// Mutually exclusive with `--all`.
    #[arg(long)]
    pub created: bool,
    /// Show bugs I'm CC'd on (instead of assigned to me).
    ///
    /// Mutually exclusive with `--all`.
    #[arg(long)]
    pub cc: bool,
    /// Show all bugs related to me (assigned + created + CC'd).
    ///
    /// Mutually exclusive with `--created` and `--cc`. Output
    /// is grouped into the three categories; `--limit` applies
    /// per category, so the total can be up to 3x the limit.
    #[arg(long, conflicts_with_all = ["created", "cc"])]
    pub all: bool,
    #[command(flatten)]
    pub filters: BugFilterArgs,
    /// Filter to bugs created at or after this date.
    ///
    /// Accepts `YYYY-MM-DD` (interpreted as 00:00:00 UTC),
    /// `YYYY-MM-DDTHH:MM:SS`, `YYYY-MM-DDTHH:MM:SSZ`, or
    /// `YYYY-MM-DDTHH:MM:SS±HH:MM`. Malformed input exits 7.
    #[arg(long, value_name = "DATE")]
    pub created_since: Option<String>,
    /// Filter to bugs last modified at or after this date.
    ///
    /// Same accepted forms as `--created-since`.
    #[arg(long, value_name = "DATE")]
    pub changed_since: Option<String>,
    /// Max results per category (assigned/created/cc).
    ///
    /// With `--all`, the limit applies independently to each of
    /// the three categories, so up to 3x this value may be
    /// returned. With `--created`, `--cc`, or no view flag,
    /// the limit applies to the single active category.
    #[arg(long, default_value = "50")]
    pub limit: u32,
    /// Print only the number of matching bugs, not the rows.
    ///
    /// Counts the distinct bugs across the active categories (deduped),
    /// bounded by the server's max-results setting, and prints just the
    /// integer (table) or `{"count": N}` (JSON).
    #[arg(long)]
    pub count: bool,
    #[command(flatten)]
    pub field_args: FieldArgs,
    #[command(flatten)]
    pub sort_args: SortArgs,
    #[command(flatten)]
    pub page_args: PageArgs,
}

#[cfg(test)]
#[path = "my_tests.rs"]
mod tests;
