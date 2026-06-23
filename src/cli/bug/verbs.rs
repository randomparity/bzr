use clap::Args;

use crate::cli::bug::CommentArgs;

pub const RESOLVE_LONG_ABOUT: &str = r#"Resolve one or more bugs (sets status RESOLVED
and a resolution).

Sugar for `bug update <IDs> --status RESOLVED --resolution <AS>`.
`--as` defaults to `FIXED`; pass another resolution (`WONTFIX`,
`INVALID`, `WORKSFORME`, `DUPLICATE`, ...) to override. Accepts
multiple IDs (batch) and the same `--comment` flags as `bug update`.

Examples:

  bzr bug resolve 12345
  bzr bug resolve 12345 12346 --as WONTFIX
  bzr bug resolve 12345 --comment "Fixed in 9.1""#;

pub const CLOSE_LONG_ABOUT: &str = r#"Close one or more bugs (sets status VERIFIED by default).

Sugar for `bug update <IDs> --status VERIFIED`. `VERIFIED` is a stock
Bugzilla 5.x closed status; override with `--status <STATUS>` for
installs that define a custom closed status such as `CLOSED`. The
target status is validated against the server's status list (exact,
case-sensitive); an unknown value exits 7. By default the bug's
existing resolution is preserved (so close an already-resolved bug);
pass `--as <RESOLUTION>` to set one when closing directly. Accepts
multiple IDs (batch) and the same `--comment` flags as `bug update`.

Examples:

  bzr bug close 12345
  bzr bug close 12345 --status CLOSED          # install with custom status
  bzr bug close 12345 12346 --as WONTFIX --comment "Out of scope""#;

pub const REOPEN_LONG_ABOUT: &str = r#"Reopen one or more bugs (sets status CONFIRMED by default).

Sugar for `bug update <IDs> --status CONFIRMED`. `CONFIRMED` is a stock
Bugzilla 5.x open status; override with `--status <STATUS>` for installs
that define a custom open status such as `REOPENED`. Bugzilla clears the
resolution automatically when moving to an open status. The target
status is validated against the server's status list (exact,
case-sensitive); an unknown value exits 7. Accepts multiple IDs (batch)
and the same `--comment` flags as `bug update`.

Examples:

  bzr bug reopen 12345
  bzr bug reopen 12345 --status REOPENED       # install with custom status
  bzr bug reopen 12345 --comment "Regressed in 9.2""#;

pub const DUP_LONG_ABOUT: &str = r#"Mark a bug as a duplicate of another bug.

Sugar for `bug update <ID> --dupe-of <TARGET>`. Bugzilla sets the
status/resolution transition (RESOLVED/DUPLICATE) automatically.
Supports the same `--comment` flags as `bug update`.

Examples:

  bzr bug dup 12345 100
  bzr bug dup 12345 100 --comment "Same root cause""#;

/// Arguments for `bug resolve`.
#[derive(Args, Debug)]
pub struct ResolveArgs {
    /// Bug ID(s) to resolve.
    #[arg(required = true, num_args = 1..)]
    pub ids: Vec<u64>,
    /// Resolution to set (default `FIXED`).
    #[arg(long = "as", value_name = "RESOLUTION", default_value = "FIXED")]
    pub as_resolution: String,
    /// Only apply the update if the bug has not changed since this time.
    #[arg(long, value_name = "TIMESTAMP")]
    pub expect_unchanged_since: Option<String>,
    #[command(flatten)]
    pub comment: CommentArgs,
}

/// Arguments for `bug close`.
#[derive(Args, Debug)]
pub struct CloseArgs {
    /// Bug ID(s) to close.
    #[arg(required = true, num_args = 1..)]
    pub ids: Vec<u64>,
    /// Status to transition to (default `VERIFIED`, a stock Bugzilla 5.x
    /// closed status). Override for installs with a custom closed status
    /// (e.g. `--status CLOSED`). Validated against the server's status list;
    /// an unknown value exits 7. Matched exactly and case-sensitively.
    #[arg(long, value_name = "STATUS", default_value = "VERIFIED")]
    pub status: String,
    /// Resolution to set when closing an unresolved bug. Omit to
    /// preserve any existing resolution.
    #[arg(long = "as", value_name = "RESOLUTION")]
    pub as_resolution: Option<String>,
    /// Only apply the update if the bug has not changed since this time.
    #[arg(long, value_name = "TIMESTAMP")]
    pub expect_unchanged_since: Option<String>,
    #[command(flatten)]
    pub comment: CommentArgs,
}

/// Arguments for `bug reopen`.
#[derive(Args, Debug)]
pub struct ReopenArgs {
    /// Bug ID(s) to reopen.
    #[arg(required = true, num_args = 1..)]
    pub ids: Vec<u64>,
    /// Status to transition to (default `CONFIRMED`, a stock Bugzilla 5.x
    /// open status). Override for installs with a custom open status
    /// (e.g. `--status REOPENED`). Validated against the server's status
    /// list; an unknown value exits 7. Matched exactly and case-sensitively.
    #[arg(long, value_name = "STATUS", default_value = "CONFIRMED")]
    pub status: String,
    /// Only apply the update if the bug has not changed since this time.
    #[arg(long, value_name = "TIMESTAMP")]
    pub expect_unchanged_since: Option<String>,
    #[command(flatten)]
    pub comment: CommentArgs,
}

/// Arguments for `bug dup`.
#[derive(Args, Debug)]
pub struct DupArgs {
    /// The duplicate bug.
    pub id: u64,
    /// The canonical bug this one duplicates.
    pub target: u64,
    /// Only apply the update if the bug has not changed since this time.
    #[arg(long, value_name = "TIMESTAMP")]
    pub expect_unchanged_since: Option<String>,
    #[command(flatten)]
    pub comment: CommentArgs,
}

#[cfg(test)]
#[path = "verbs_tests.rs"]
mod tests;
