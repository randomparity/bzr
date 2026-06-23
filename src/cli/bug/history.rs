use clap::Args;

pub const LONG_ABOUT: &str = r"Show the change history for a single bug.

Prints every recorded change to the bug's fields (status,
assignee, comments added, attachments, etc.) in chronological
order, including the user who made each change. Use `--since`
(ISO 8601 date or datetime) to limit the output to changes
after a given point.

Examples:

  bzr bug history 12345
  bzr bug history 12345 --since 2026-01-01
  bzr bug history 12345 --since 2026-04-15T00:00:00Z --json

See bzr-bug-view(1) for the current state of a bug.";

/// Arguments for `bug history`.
#[derive(Args, Debug)]
pub(crate) struct HistoryArgs {
    /// Bug ID
    pub id: u64,
    /// Only show changes after this date (ISO 8601)
    #[arg(long)]
    pub since: Option<String>,
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
