#![expect(clippy::unwrap_used, clippy::panic)]

use super::ListArgs;
use crate::cli::bug::BugAction;
use crate::cli::{Cli, Commands};
use crate::types::common::SortDirection;
use clap::error::ErrorKind;
use clap::Parser as _;

fn list_args(args: &[&str]) -> ListArgs {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Bug {
            action: BugAction::List(list),
        } => list,
        _ => panic!("expected Commands::Bug {{ action: List }}"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_list_defaults() {
    let list = list_args(&["bzr", "bug", "list"]);
    assert_eq!(list.limit, 50);
    assert!(!list.count);
    assert!(list.id.is_empty());
    assert!(list.alias.is_none());
    assert!(list.summary.is_none());
    assert!(list.created_since.is_none());
    assert!(list.changed_since.is_none());
    assert!(list.sort_args.sort.is_none());
    assert_eq!(list.sort_args.order, SortDirection::Asc);
    assert!(list.page_args.offset.is_none());
    assert!(!list.page_args.paginate);
}

#[test]
fn parse_list_core_and_actor_filters() {
    let list = list_args(&[
        "bzr",
        "bug",
        "list",
        "--product",
        "Firefox",
        "--component",
        "Core",
        "--status",
        "NEW",
        "--status",
        "ASSIGNED",
        "--priority",
        "high",
        "--severity",
        "major",
        "--assignee",
        "me@example.com",
        "--creator",
        "you@example.com",
    ]);
    assert_eq!(list.filters.product, vec!["Firefox"]);
    assert_eq!(list.filters.component, vec!["Core"]);
    assert_eq!(list.filters.status, vec!["NEW", "ASSIGNED"]);
    assert_eq!(list.filters.priority, vec!["high"]);
    assert_eq!(list.filters.severity, vec!["major"]);
    assert_eq!(list.actor_filters.assignee, vec!["me@example.com"]);
    assert_eq!(list.actor_filters.creator, vec!["you@example.com"]);
}

#[test]
fn parse_list_bzl_parity_field_filters() {
    let list = list_args(&[
        "bzr",
        "bug",
        "list",
        "--whiteboard",
        "needs-triage",
        "--target-milestone",
        "9.0",
        "--version",
        "rawhide",
        "--op-sys",
        "Linux",
        "--platform",
        "x86_64",
        "--resolution",
        "FIXED",
        "--qa-contact",
        "qa@example.com",
        "--url",
        "https://example.com",
    ]);
    assert_eq!(list.filters.whiteboard, vec!["needs-triage"]);
    assert_eq!(list.filters.target_milestone, vec!["9.0"]);
    assert_eq!(list.filters.version, vec!["rawhide"]);
    assert_eq!(list.filters.op_sys, vec!["Linux"]);
    assert_eq!(list.filters.platform, vec!["x86_64"]);
    assert_eq!(list.filters.resolution, vec!["FIXED"]);
    assert_eq!(list.filters.qa_contact, vec!["qa@example.com"]);
    assert_eq!(list.filters.url, vec!["https://example.com"]);
}

#[test]
fn parse_list_negated_filter_value_is_passed_through() {
    let list = list_args(&["bzr", "bug", "list", "--status", "!CLOSED"]);
    assert_eq!(list.filters.status, vec!["!CLOSED"]);
}

#[test]
fn parse_list_id_alias_summary_count() {
    let list = list_args(&[
        "bzr",
        "bug",
        "list",
        "--id",
        "100",
        "--id",
        "101",
        "--alias",
        "my-alias",
        "--summary",
        "kernel panic",
        "--count",
    ]);
    assert_eq!(list.id, vec![100, 101]);
    assert_eq!(list.alias.as_deref(), Some("my-alias"));
    assert_eq!(list.summary.as_deref(), Some("kernel panic"));
    assert!(list.count);
}

#[test]
fn parse_list_field_selection_and_dates() {
    let list = list_args(&[
        "bzr",
        "bug",
        "list",
        "--fields",
        "id,summary",
        "--exclude-fields",
        "cc",
        "--created-since",
        "2026-01-01",
        "--changed-since",
        "2026-05-01",
        "--limit",
        "25",
    ]);
    assert_eq!(list.field_args.fields.as_deref(), Some("id,summary"));
    assert_eq!(list.field_args.exclude_fields.as_deref(), Some("cc"));
    assert_eq!(list.created_since.as_deref(), Some("2026-01-01"));
    assert_eq!(list.changed_since.as_deref(), Some("2026-05-01"));
    assert_eq!(list.limit, 25);
}

#[test]
fn parse_list_sort_and_offset() {
    let list = list_args(&[
        "bzr",
        "bug",
        "list",
        "--sort",
        "last_change_time",
        "--order",
        "desc",
        "--offset",
        "100",
    ]);
    assert_eq!(list.sort_args.sort.as_deref(), Some("last_change_time"));
    assert_eq!(list.sort_args.order, SortDirection::Desc);
    assert_eq!(list.page_args.offset, Some(100));
}

#[test]
fn parse_list_paginate() {
    let list = list_args(&["bzr", "bug", "list", "--paginate"]);
    assert!(list.page_args.paginate);
    assert!(list.page_args.offset.is_none());
}

#[test]
fn list_rejects_offset_with_paginate() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "list", "--offset", "10", "--paginate"]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn list_rejects_non_numeric_id() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "list", "--id", "not-a-number"]),
        ErrorKind::ValueValidation
    );
}

#[test]
fn list_rejects_invalid_order() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "list", "--order", "sideways"]),
        ErrorKind::ValueValidation
    );
}
