use clap::Args;

use crate::cli::bug::{BugActorFilterArgs, BugFilterArgs, FieldArgs, PageArgs, SortArgs};

pub const LONG_ABOUT: &str = r#"List bugs that match the given filters.

Filter flags (`--product`, `--component`, `--status`,
`--assignee`, `--creator`, `--priority`, `--severity`) are
repeatable for OR semantics within a category and AND across
categories. Prefix any filter value with `!` to invert it
(e.g. `--status '!CLOSED'`).

`--summary` matches a substring against the bug's Summary field
across all bug states (open and closed). It is the structured
counterpart to `bzr bug search`, which uses Bugzilla's
quicksearch syntax and defaults to open bugs only.

`--limit` defaults to 50; raise it for broader scans, but very
large values may exceed the server's max-results setting and
return a truncated list. `--fields` and `--exclude-fields` control
which fields are requested from the server; in table output they
select and remove columns (in the given order). Under `--json` the
output object is trimmed to the selected fields (gh-style):
`--fields summary` returns `{"summary": ...}` with no `id` unless you
ask for it, and `--exclude-fields id` drops `id`. Built-in fields and
Bugzilla custom fields named `cf_*` are valid; custom fields are shown
when requested and returned by the server. Unknown non-custom fields
are skipped with a warning, or rejected if nothing known remains.

`--created-since` / `--changed-since` filter by Bugzilla's
`creation_time` / `last_change_time` fields. Both accept ISO
8601 (`YYYY-MM-DDTHH:MM:SSZ`) or a bare `YYYY-MM-DD` (treated
as 00:00:00 UTC). Malformed input exits 7 before any network
call.

Eight additional field filters from bzl-parity issue #158:
`--whiteboard`, `--target-milestone`, `--version`, `--op-sys`,
`--platform`, `--resolution`, `--qa-contact`, `--url`. All
repeatable for OR within a field; AND across fields. Prefix
with `!` to invert. `--whiteboard` and `--url` are substring
matches (negation uses `notsubstring`); the other six are
exact match (negation uses `notequals`).

Examples:

  bzr bug list --product Firefox --status NEW --limit 25
  bzr bug list --assignee me@example.com --status '!CLOSED'
  bzr bug list --summary "kernel panic" --product Kernel
  bzr bug list --id 100,101,102
  bzr bug list --product Firefox --changed-since 2026-04-01

See bzr-bug-search(1) for free-text search, bzr-bug-my(1) for
caller-relative views, and bzr-query(1) for saving a filter
combination by name."#;

/// Arguments for `bug list`.
#[derive(Args, Debug, Default)]
pub struct ListArgs {
    #[command(flatten)]
    pub filters: BugFilterArgs,
    #[command(flatten)]
    pub actor_filters: BugActorFilterArgs,
    /// Filter by bug IDs
    #[arg(long)]
    pub id: Vec<u64>,
    /// Filter by alias
    #[arg(long)]
    pub alias: Option<String>,
    /// Substring match on the Summary field (matches across all bug states)
    #[arg(long)]
    pub summary: Option<String>,
    /// Max number of results
    #[arg(long, default_value = "50")]
    pub limit: u32,
    /// Print only the number of matching bugs, not the rows.
    ///
    /// Counts all matches (bounded by the server's max-results setting)
    /// and prints just the integer (table) or `{"count": N}` (JSON).
    /// `--limit` and `--sort` are ignored, and `--fields` does not affect
    /// the count (though an invalid `--fields` value is still rejected).
    #[arg(long)]
    pub count: bool,
    #[command(flatten)]
    pub field_args: FieldArgs,
    #[command(flatten)]
    pub sort_args: SortArgs,
    #[command(flatten)]
    pub page_args: PageArgs,
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
}
