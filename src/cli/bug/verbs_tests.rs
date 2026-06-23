#![expect(clippy::unwrap_used, clippy::panic)]

use crate::cli::bug::BugAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn bug_action(args: &[&str]) -> BugAction {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Bug { action } => action,
        _ => panic!("expected Commands::Bug"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_resolve_defaults_to_fixed() {
    match bug_action(&["bzr", "bug", "resolve", "12345"]) {
        BugAction::Resolve(resolve) => {
            assert_eq!(resolve.ids, vec![12345]);
            assert_eq!(resolve.as_resolution, "FIXED");
            assert!(resolve.comment.comment.is_none());
        }
        _ => panic!("expected Resolve"),
    }
}

#[test]
fn parse_resolve_with_as_and_comment_and_batch() {
    match bug_action(&[
        "bzr",
        "bug",
        "resolve",
        "12345",
        "12346",
        "--as",
        "WONTFIX",
        "--comment",
        "out of scope",
        "--comment-private",
    ]) {
        BugAction::Resolve(resolve) => {
            assert_eq!(resolve.ids, vec![12345, 12346]);
            assert_eq!(resolve.as_resolution, "WONTFIX");
            assert_eq!(resolve.comment.comment.as_deref(), Some("out of scope"));
            assert!(resolve.comment.comment_private);
        }
        _ => panic!("expected Resolve"),
    }
}

#[test]
fn parse_close_default_status_verified() {
    match bug_action(&["bzr", "bug", "close", "12345"]) {
        BugAction::Close(close) => {
            assert_eq!(close.ids, vec![12345]);
            assert_eq!(close.status, "VERIFIED");
            assert!(close.as_resolution.is_none());
        }
        _ => panic!("expected Close"),
    }
}

#[test]
fn parse_close_status_override_and_resolution() {
    match bug_action(&[
        "bzr", "bug", "close", "12345", "--status", "CLOSED", "--as", "WONTFIX",
    ]) {
        BugAction::Close(close) => {
            assert_eq!(close.status, "CLOSED");
            assert_eq!(close.as_resolution.as_deref(), Some("WONTFIX"));
        }
        _ => panic!("expected Close"),
    }
}

#[test]
fn parse_reopen_default_status_confirmed() {
    match bug_action(&["bzr", "bug", "reopen", "12345"]) {
        BugAction::Reopen(reopen) => {
            assert_eq!(reopen.ids, vec![12345]);
            assert_eq!(reopen.status, "CONFIRMED");
        }
        _ => panic!("expected Reopen"),
    }
}

#[test]
fn parse_reopen_status_override() {
    match bug_action(&["bzr", "bug", "reopen", "12345", "--status", "REOPENED"]) {
        BugAction::Reopen(reopen) => assert_eq!(reopen.status, "REOPENED"),
        _ => panic!("expected Reopen"),
    }
}

#[test]
fn parse_dup_binds_id_and_target() {
    match bug_action(&[
        "bzr",
        "bug",
        "dup",
        "12345",
        "100",
        "--comment",
        "same cause",
    ]) {
        BugAction::Dup(dup) => {
            assert_eq!(dup.id, 12345);
            assert_eq!(dup.target, 100);
            assert_eq!(dup.comment.comment.as_deref(), Some("same cause"));
        }
        _ => panic!("expected Dup"),
    }
}

#[test]
fn parse_verb_expect_unchanged_since() {
    match bug_action(&[
        "bzr",
        "bug",
        "close",
        "12345",
        "--expect-unchanged-since",
        "2026-05-01T00:00:00Z",
    ]) {
        BugAction::Close(close) => assert_eq!(
            close.expect_unchanged_since.as_deref(),
            Some("2026-05-01T00:00:00Z")
        ),
        _ => panic!("expected Close"),
    }
}

#[test]
fn resolve_rejects_comment_with_comment_file() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "bug",
            "resolve",
            "12345",
            "--comment",
            "body",
            "--comment-file",
            "/tmp/c.txt",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn resolve_requires_at_least_one_id() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "resolve", "--as", "FIXED"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn dup_requires_both_id_and_target() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "dup", "12345"]),
        ErrorKind::MissingRequiredArgument
    );
}
