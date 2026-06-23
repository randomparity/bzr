#![expect(clippy::unwrap_used, clippy::panic)]

use super::UpdateArgs;
use crate::cli::bug::BugAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn update_args(args: &[&str]) -> UpdateArgs {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Bug {
            action: BugAction::Update(update),
        } => update,
        _ => panic!("expected Commands::Bug {{ action: Update }}"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_update_multiple_ids_and_scalar_fields() {
    let update = update_args(&[
        "bzr",
        "bug",
        "update",
        "100",
        "200",
        "300",
        "--status",
        "RESOLVED",
        "--resolution",
        "FIXED",
        "--assignee",
        "me@example.com",
        "--priority",
        "high",
        "--severity",
        "major",
        "--summary",
        "new summary",
        "--whiteboard",
        "wb",
        "--url",
        "https://example.com",
        "--target-milestone",
        "9.0",
    ]);
    assert_eq!(update.ids, vec![100, 200, 300]);
    assert_eq!(update.status.as_deref(), Some("RESOLVED"));
    assert_eq!(update.resolution.as_deref(), Some("FIXED"));
    assert_eq!(update.assignee.as_deref(), Some("me@example.com"));
    assert_eq!(update.priority.as_deref(), Some("high"));
    assert_eq!(update.severity.as_deref(), Some("major"));
    assert_eq!(update.summary.as_deref(), Some("new summary"));
    assert_eq!(update.whiteboard.as_deref(), Some("wb"));
    assert_eq!(update.url.as_deref(), Some("https://example.com"));
    assert_eq!(update.target_milestone.as_deref(), Some("9.0"));
}

#[test]
fn parse_update_dupe_of() {
    let update = update_args(&["bzr", "bug", "update", "100", "--dupe-of", "200"]);
    assert_eq!(update.dupe_of, Some(200));
}

#[test]
fn parse_update_time_tracking_and_resets() {
    let update = update_args(&[
        "bzr",
        "bug",
        "update",
        "100",
        "--estimated-time",
        "8.5",
        "--remaining-time",
        "2.0",
        "--work-time",
        "1.5",
        "--reset-assigned-to",
        "--reset-qa-contact",
    ]);
    assert_eq!(update.estimated_time, Some(8.5));
    assert_eq!(update.remaining_time, Some(2.0));
    assert_eq!(update.work_time, Some(1.5));
    assert!(update.reset_assigned_to);
    assert!(update.reset_qa_contact);
}

#[test]
fn parse_update_list_add_remove_comma_lists() {
    let update = update_args(&[
        "bzr",
        "bug",
        "update",
        "100",
        "--blocks-add",
        "200,201",
        "--blocks-remove",
        "99",
        "--depends-on-add",
        "300",
        "--keywords-add",
        "regression,crash",
        "--cc-add",
        "a@example.com,b@example.com",
        "--groups-remove",
        "secure",
    ]);
    assert_eq!(update.blocks_add, vec![200, 201]);
    assert_eq!(update.blocks_remove, vec![99]);
    assert_eq!(update.depends_on_add, vec![300]);
    assert_eq!(update.keywords_add, vec!["regression", "crash"]);
    assert_eq!(update.cc_add, vec!["a@example.com", "b@example.com"]);
    assert_eq!(update.groups_remove, vec!["secure"]);
}

#[test]
fn parse_update_see_also_repeats_without_comma_split() {
    // see-also URLs may contain commas, so the flag repeats instead of
    // splitting; a single value with a comma stays one entry.
    let update = update_args(&[
        "bzr",
        "bug",
        "update",
        "100",
        "--see-also-add",
        "https://example.com/a,b",
        "--see-also-add",
        "https://example.com/c",
        "--see-also-remove",
        "https://example.com/old",
    ]);
    assert_eq!(
        update.see_also_add,
        vec!["https://example.com/a,b", "https://example.com/c"]
    );
    assert_eq!(update.see_also_remove, vec!["https://example.com/old"]);
}

#[test]
fn parse_update_comment_private_and_flag() {
    let update = update_args(&[
        "bzr",
        "bug",
        "update",
        "100",
        "--comment",
        "fixed it",
        "--comment-private",
        "--flag",
        "review+",
        "--flag",
        "qe?",
    ]);
    assert_eq!(update.comment.as_deref(), Some("fixed it"));
    assert!(update.comment_private);
    assert_eq!(update.flag, vec!["review+", "qe?"]);
}

#[test]
fn parse_update_from_json_without_positional_ids() {
    let update = update_args(&["bzr", "bug", "update", "--from-json", "-"]);
    assert_eq!(update.from_json.as_deref(), Some("-"));
    assert!(update.ids.is_empty());
}

#[test]
fn parse_update_expect_unchanged_since() {
    let update = update_args(&[
        "bzr",
        "bug",
        "update",
        "100",
        "--expect-unchanged-since",
        "2026-05-01T00:00:00Z",
    ]);
    assert_eq!(
        update.expect_unchanged_since.as_deref(),
        Some("2026-05-01T00:00:00Z")
    );
}

#[test]
fn update_requires_id_without_from_json() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "update", "--status", "RESOLVED"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn update_rejects_status_with_dupe_of() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "bug",
            "update",
            "100",
            "--status",
            "RESOLVED",
            "--dupe-of",
            "200",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn update_rejects_resolution_with_dupe_of() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "bug",
            "update",
            "100",
            "--resolution",
            "FIXED",
            "--dupe-of",
            "200",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn update_rejects_comment_with_comment_file() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "bug",
            "update",
            "100",
            "--comment",
            "body",
            "--comment-file",
            "/tmp/c.txt",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn update_rejects_non_numeric_id() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "update", "not-a-number"]),
        ErrorKind::ValueValidation
    );
}
