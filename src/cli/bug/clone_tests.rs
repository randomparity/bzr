#![expect(clippy::unwrap_used, clippy::panic)]

use super::CloneArgs;
use crate::cli::bug::BugAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn clone_args(args: &[&str]) -> CloneArgs {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Bug {
            action: BugAction::Clone(clone),
        } => clone,
        _ => panic!("expected Commands::Bug {{ action: Clone }}"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_clone_minimal_binds_source_id() {
    let clone = clone_args(&["bzr", "bug", "clone", "12345"]);
    assert_eq!(clone.id, "12345");
    assert!(clone.summary.is_none());
    assert!(!clone.no_comment);
    assert!(!clone.add_depends_on);
    assert!(!clone.add_blocks);
    assert!(!clone.no_cc);
    assert!(!clone.no_keywords);
}

#[test]
fn parse_clone_field_overrides() {
    let clone = clone_args(&[
        "bzr",
        "bug",
        "clone",
        "12345",
        "--summary",
        "Backport",
        "--product",
        "RHEL",
        "--component",
        "kernel",
        "--version",
        "9.4",
        "--description",
        "see parent",
        "--priority",
        "high",
        "--severity",
        "major",
        "--assignee",
        "me@example.com",
        "--op-sys",
        "Linux",
        "--platform",
        "x86_64",
    ]);
    assert_eq!(clone.summary.as_deref(), Some("Backport"));
    assert_eq!(clone.product.as_deref(), Some("RHEL"));
    assert_eq!(clone.component.as_deref(), Some("kernel"));
    assert_eq!(clone.version.as_deref(), Some("9.4"));
    assert_eq!(clone.description.as_deref(), Some("see parent"));
    assert_eq!(clone.priority.as_deref(), Some("high"));
    assert_eq!(clone.severity.as_deref(), Some("major"));
    assert_eq!(clone.assignee.as_deref(), Some("me@example.com"));
    assert_eq!(clone.op_sys.as_deref(), Some("Linux"));
    assert_eq!(clone.platform.as_deref(), Some("x86_64"));
}

#[test]
fn parse_clone_accepts_hidden_legacy_platform_alias() {
    let clone = clone_args(&["bzr", "bug", "clone", "42", "--rep-platform", "x86_64"]);
    assert_eq!(clone.platform.as_deref(), Some("x86_64"));
}

#[test]
fn parse_clone_link_and_skip_switches() {
    let clone = clone_args(&[
        "bzr",
        "bug",
        "clone",
        "12345",
        "--no-comment",
        "--add-depends-on",
        "--add-blocks",
    ]);
    assert!(clone.no_comment);
    assert!(clone.add_depends_on);
    assert!(clone.add_blocks);
}

#[test]
fn parse_clone_create_field_overrides() {
    let clone = clone_args(&[
        "bzr",
        "bug",
        "clone",
        "12345",
        "--url",
        "https://example.com",
        "--whiteboard",
        "wb",
        "--target-milestone",
        "9.0",
        "--deadline",
        "2026-12-31",
        "--groups",
        "secure",
        "--flag",
        "review?",
    ]);
    let fields = clone.create_fields;
    assert_eq!(fields.url.as_deref(), Some("https://example.com"));
    assert_eq!(fields.whiteboard.as_deref(), Some("wb"));
    assert_eq!(fields.target_milestone.as_deref(), Some("9.0"));
    assert_eq!(fields.deadline.as_deref(), Some("2026-12-31"));
    assert_eq!(fields.groups, vec!["secure"]);
    assert_eq!(fields.flag, vec!["review?"]);
}

#[test]
fn clone_rejects_cc_override_with_no_cc() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "bug",
            "clone",
            "12345",
            "--cc",
            "a@example.com",
            "--no-cc",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn clone_rejects_keywords_override_with_no_keywords() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "bug",
            "clone",
            "12345",
            "--keywords",
            "regression",
            "--no-keywords",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn clone_requires_source_id() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "clone"]),
        ErrorKind::MissingRequiredArgument
    );
}
