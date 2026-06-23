#![expect(clippy::unwrap_used, clippy::panic)]

use super::CreateArgs;
use crate::cli::bug::BugAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn create_args(args: &[&str]) -> CreateArgs {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Bug {
            action: BugAction::Create(create),
        } => create,
        _ => panic!("expected Commands::Bug {{ action: Create }}"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_create_core_fields() {
    let create = create_args(&[
        "bzr",
        "bug",
        "create",
        "--product",
        "Fedora",
        "--component",
        "kernel",
        "--summary",
        "Boot failure",
        "--version",
        "rawhide",
        "--description",
        "hangs at initramfs",
        "--priority",
        "high",
        "--severity",
        "major",
        "--assignee",
        "me@example.com",
        "--op-sys",
        "Linux",
        "--rep-platform",
        "x86_64",
    ]);
    assert_eq!(create.product.as_deref(), Some("Fedora"));
    assert_eq!(create.component.as_deref(), Some("kernel"));
    assert_eq!(create.summary.as_deref(), Some("Boot failure"));
    assert_eq!(create.version.as_deref(), Some("rawhide"));
    assert_eq!(create.description.as_deref(), Some("hangs at initramfs"));
    assert_eq!(create.priority.as_deref(), Some("high"));
    assert_eq!(create.severity.as_deref(), Some("major"));
    assert_eq!(create.assignee.as_deref(), Some("me@example.com"));
    assert_eq!(create.op_sys.as_deref(), Some("Linux"));
    assert_eq!(create.rep_platform.as_deref(), Some("x86_64"));
    assert!(create.template.is_none());
    assert!(create.from_json.is_none());
}

#[test]
fn parse_create_blocks_and_depends_on_comma_lists() {
    let create = create_args(&[
        "bzr",
        "bug",
        "create",
        "--product",
        "P",
        "--component",
        "C",
        "--summary",
        "S",
        "--blocks",
        "100,101",
        "--depends-on",
        "200,201",
    ]);
    assert_eq!(create.blocks, vec![100, 101]);
    assert_eq!(create.depends_on, vec![200, 201]);
}

#[test]
fn parse_create_field_args_metadata() {
    let create = create_args(&[
        "bzr",
        "bug",
        "create",
        "--product",
        "P",
        "--component",
        "C",
        "--summary",
        "S",
        "--alias",
        "my-alias",
        "--url",
        "https://example.com",
        "--whiteboard",
        "needs-triage",
        "--target-milestone",
        "9.0",
        "--deadline",
        "2026-12-31",
        "--cc",
        "a@example.com,b@example.com",
        "--keywords",
        "regression,crash",
        "--groups",
        "secure",
        "--flag",
        "review?",
        "--flag",
        "qe+",
    ]);
    let fields = create.create_fields;
    assert_eq!(fields.alias.as_deref(), Some("my-alias"));
    assert_eq!(fields.url.as_deref(), Some("https://example.com"));
    assert_eq!(fields.whiteboard.as_deref(), Some("needs-triage"));
    assert_eq!(fields.target_milestone.as_deref(), Some("9.0"));
    assert_eq!(fields.deadline.as_deref(), Some("2026-12-31"));
    assert_eq!(fields.cc, vec!["a@example.com", "b@example.com"]);
    assert_eq!(fields.keywords, vec!["regression", "crash"]);
    assert_eq!(fields.groups, vec!["secure"]);
    assert_eq!(fields.flag, vec!["review?", "qe+"]);
}

#[test]
fn parse_create_from_json_stdin() {
    let create = create_args(&["bzr", "bug", "create", "--from-json", "-"]);
    assert_eq!(create.from_json.as_deref(), Some("-"));
}

#[test]
fn parse_create_template() {
    let create = create_args(&[
        "bzr",
        "bug",
        "create",
        "--template",
        "security-bug",
        "--summary",
        "XSS in login",
    ]);
    assert_eq!(create.template.as_deref(), Some("security-bug"));
}

#[test]
fn create_rejects_from_json_with_template() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "bug",
            "create",
            "--from-json",
            "-",
            "--template",
            "t",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn create_rejects_description_with_description_file() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "bug",
            "create",
            "--product",
            "P",
            "--component",
            "C",
            "--summary",
            "S",
            "--description",
            "literal",
            "--description-file",
            "/tmp/desc.txt",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn create_rejects_non_numeric_blocks() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "bug",
            "create",
            "--product",
            "P",
            "--component",
            "C",
            "--summary",
            "S",
            "--blocks",
            "not-a-number",
        ]),
        ErrorKind::ValueValidation
    );
}
