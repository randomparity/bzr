#![expect(clippy::unwrap_used, clippy::panic)]

use super::ViewArgs;
use crate::cli::bug::BugAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn view_args(args: &[&str]) -> ViewArgs {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Bug {
            action: BugAction::View(view),
        } => view,
        _ => panic!("expected Commands::Bug {{ action: View }}"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_view_single_id_defaults() {
    let view = view_args(&["bzr", "bug", "view", "12345"]);
    assert_eq!(view.ids, vec!["12345"]);
    assert!(!view.permissive);
    assert!(!view.web);
    assert!(view.field_args.fields.is_none());
}

#[test]
fn parse_view_mixed_ids_and_aliases() {
    let view = view_args(&["bzr", "bug", "view", "12345", "my-alias", "12347"]);
    assert_eq!(view.ids, vec!["12345", "my-alias", "12347"]);
}

#[test]
fn parse_view_permissive_with_fields() {
    let view = view_args(&[
        "bzr",
        "bug",
        "view",
        "12345",
        "12346",
        "--permissive",
        "--fields",
        "id,summary,status",
    ]);
    assert!(view.permissive);
    assert_eq!(view.field_args.fields.as_deref(), Some("id,summary,status"));
}

#[test]
fn parse_view_web_flag() {
    let view = view_args(&["bzr", "bug", "view", "12345", "--web"]);
    assert!(view.web);
}

#[test]
fn parse_view_exclude_fields() {
    let view = view_args(&["bzr", "bug", "view", "12345", "--exclude-fields", "cc"]);
    assert_eq!(view.field_args.exclude_fields.as_deref(), Some("cc"));
}

#[test]
fn view_requires_at_least_one_id() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "view"]),
        ErrorKind::MissingRequiredArgument
    );
}
