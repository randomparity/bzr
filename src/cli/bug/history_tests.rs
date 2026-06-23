#![expect(clippy::unwrap_used, clippy::panic)]

use super::HistoryArgs;
use crate::cli::bug::BugAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn history_args(args: &[&str]) -> HistoryArgs {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Bug {
            action: BugAction::History(history),
        } => history,
        _ => panic!("expected Commands::Bug {{ action: History }}"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_history_id_only() {
    let history = history_args(&["bzr", "bug", "history", "12345"]);
    assert_eq!(history.id, 12345);
    assert!(history.since.is_none());
}

#[test]
fn parse_history_with_since() {
    let history = history_args(&[
        "bzr",
        "bug",
        "history",
        "12345",
        "--since",
        "2026-04-15T00:00:00Z",
    ]);
    assert_eq!(history.id, 12345);
    assert_eq!(history.since.as_deref(), Some("2026-04-15T00:00:00Z"));
}

#[test]
fn history_requires_id() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "history"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn history_rejects_non_numeric_id() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "history", "not-a-number"]),
        ErrorKind::ValueValidation
    );
}
