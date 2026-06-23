#![expect(clippy::unwrap_used, clippy::panic)]

use super::MyArgs;
use crate::cli::bug::BugAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn my_args(args: &[&str]) -> MyArgs {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Bug {
            action: BugAction::My(my),
        } => my,
        _ => panic!("expected Commands::Bug {{ action: My }}"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_my_defaults_to_assigned() {
    let my = my_args(&["bzr", "bug", "my"]);
    assert!(!my.created);
    assert!(!my.cc);
    assert!(!my.all);
    assert!(!my.count);
    assert_eq!(my.limit, 50);
}

#[test]
fn parse_my_created_view() {
    let my = my_args(&["bzr", "bug", "my", "--created", "--status", "NEW"]);
    assert!(my.created);
    assert_eq!(my.filters.status, vec!["NEW"]);
}

#[test]
fn parse_my_cc_view() {
    let my = my_args(&["bzr", "bug", "my", "--cc"]);
    assert!(my.cc);
}

#[test]
fn parse_my_all_view_with_limit() {
    let my = my_args(&["bzr", "bug", "my", "--all", "--limit", "25"]);
    assert!(my.all);
    assert_eq!(my.limit, 25);
}

#[test]
fn parse_my_shared_filters_and_dates() {
    let my = my_args(&[
        "bzr",
        "bug",
        "my",
        "--product",
        "Firefox",
        "--created-since",
        "2026-01-01",
        "--changed-since",
        "2026-05-01",
        "--count",
    ]);
    assert_eq!(my.filters.product, vec!["Firefox"]);
    assert_eq!(my.created_since.as_deref(), Some("2026-01-01"));
    assert_eq!(my.changed_since.as_deref(), Some("2026-05-01"));
    assert!(my.count);
}

#[test]
fn my_rejects_all_with_created() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "my", "--all", "--created"]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn my_rejects_all_with_cc() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "my", "--all", "--cc"]),
        ErrorKind::ArgumentConflict
    );
}
