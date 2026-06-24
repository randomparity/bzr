#![expect(clippy::unwrap_used, clippy::panic)]

use super::{DeleteArgs, QueryAction, RunArgs, SaveArgs, ShowArgs, UpdateArgs};
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn query_action(args: &[&str]) -> QueryAction {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Query { action } => action,
        _ => panic!("expected Commands::Query"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_query_save_filter_kind_maps_filters() {
    match query_action(&[
        "bzr",
        "query",
        "save",
        "fx",
        "--product",
        "Firefox",
        "--status",
        "NEW",
        "--assignee",
        "me@example.com",
    ]) {
        QueryAction::Save(SaveArgs {
            name,
            from_url,
            search,
            filters,
            actor_filters,
            ..
        }) => {
            assert_eq!(name, "fx");
            assert!(from_url.is_none());
            assert!(search.is_none());
            assert_eq!(filters.product, vec!["Firefox"]);
            assert_eq!(filters.status, vec!["NEW"]);
            assert_eq!(actor_filters.assignee, vec!["me@example.com"]);
        }
        _ => panic!("expected Save"),
    }
}

#[test]
fn parse_query_save_repeats_filter_flag() {
    match query_action(&[
        "bzr",
        "query",
        "save",
        "fx",
        "--product",
        "Firefox",
        "--product",
        "Core",
    ]) {
        QueryAction::Save(SaveArgs { filters, .. }) => {
            assert_eq!(filters.product, vec!["Firefox", "Core"]);
        }
        _ => panic!("expected Save"),
    }
}

#[test]
fn parse_query_save_search_kind() {
    match query_action(&[
        "bzr",
        "query",
        "save",
        "crashes",
        "--search",
        "crash in tab",
    ]) {
        QueryAction::Save(SaveArgs {
            search, from_url, ..
        }) => {
            assert_eq!(search.as_deref(), Some("crash in tab"));
            assert!(from_url.is_none());
        }
        _ => panic!("expected Save"),
    }
}

#[test]
fn parse_query_save_from_url_kind_with_stored_overrides() {
    match query_action(&[
        "bzr",
        "query",
        "save",
        "my",
        "--from-url",
        "https://bz/buglist.cgi?product=Firefox",
        "--limit",
        "50",
        "--fields",
        "id,summary",
    ]) {
        QueryAction::Save(SaveArgs {
            from_url,
            limit,
            fields,
            ..
        }) => {
            assert_eq!(
                from_url.as_deref(),
                Some("https://bz/buglist.cgi?product=Firefox")
            );
            assert_eq!(limit, Some(50));
            assert_eq!(fields.as_deref(), Some("id,summary"));
        }
        _ => panic!("expected Save"),
    }
}

#[test]
fn parse_query_save_requires_name() {
    assert_eq!(
        parse_error_kind(&["bzr", "query", "save"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn parse_query_save_rejects_search_with_filter_flag() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "query",
            "save",
            "x",
            "--search",
            "foo",
            "--product",
            "Firefox",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parse_query_save_rejects_from_url_with_search() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "query",
            "save",
            "x",
            "--from-url",
            "https://bz",
            "--search",
            "foo",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parse_query_save_rejects_from_url_with_filter_flag() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "query",
            "save",
            "x",
            "--from-url",
            "https://bz",
            "--product",
            "Firefox",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parse_query_list() {
    assert!(matches!(
        query_action(&["bzr", "query", "list"]),
        QueryAction::List
    ));
}

#[test]
fn parse_query_show_binds_name() {
    match query_action(&["bzr", "query", "show", "fx"]) {
        QueryAction::Show(ShowArgs { name }) => assert_eq!(name, "fx"),
        _ => panic!("expected Show"),
    }
}

#[test]
fn parse_query_delete_binds_name() {
    match query_action(&["bzr", "query", "delete", "fx"]) {
        QueryAction::Delete(DeleteArgs { name }) => assert_eq!(name, "fx"),
        _ => panic!("expected Delete"),
    }
}

#[test]
fn parse_query_update_merges_filter_and_clear() {
    match query_action(&[
        "bzr", "query", "update", "fx", "--status", "ASSIGNED", "--limit", "100", "--clear",
        "severity",
    ]) {
        QueryAction::Update(UpdateArgs {
            name,
            filters,
            limit,
            clear,
            ..
        }) => {
            assert_eq!(name, "fx");
            assert_eq!(filters.status, vec!["ASSIGNED"]);
            assert_eq!(limit, Some(100));
            assert_eq!(clear, vec!["severity"]);
        }
        _ => panic!("expected Update"),
    }
}

#[test]
fn parse_query_update_replaces_search() {
    match query_action(&["bzr", "query", "update", "fx", "--search", "new terms"]) {
        QueryAction::Update(UpdateArgs { search, .. }) => {
            assert_eq!(search.as_deref(), Some("new terms"));
        }
        _ => panic!("expected Update"),
    }
}

#[test]
fn parse_query_update_rejects_from_url_with_clear() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "query",
            "update",
            "fx",
            "--from-url",
            "https://bz",
            "--clear",
            "severity",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parse_query_update_rejects_from_url_with_filter_flag() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "query",
            "update",
            "fx",
            "--from-url",
            "https://bz",
            "--product",
            "Firefox",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn parse_query_run_minimal_defaults() {
    match query_action(&["bzr", "query", "run", "fx"]) {
        QueryAction::Run(RunArgs {
            name,
            limit,
            count,
            fields,
            exclude_fields,
            server,
            url,
            page_args,
            ..
        }) => {
            assert_eq!(name, "fx");
            assert!(limit.is_none());
            assert!(!count);
            assert!(fields.is_none());
            assert!(exclude_fields.is_none());
            assert!(server.is_none());
            assert!(url.is_empty());
            assert!(page_args.offset.is_none());
            assert!(!page_args.paginate);
        }
        _ => panic!("expected Run"),
    }
}

#[test]
fn parse_query_run_overrides() {
    match query_action(&[
        "bzr",
        "query",
        "run",
        "fx",
        "--limit",
        "10",
        "--server",
        "staging",
        "--fields",
        "id,status",
    ]) {
        QueryAction::Run(RunArgs {
            limit,
            server,
            fields,
            ..
        }) => {
            assert_eq!(limit, Some(10));
            assert_eq!(server.as_deref(), Some("staging"));
            assert_eq!(fields.as_deref(), Some("id,status"));
        }
        _ => panic!("expected Run"),
    }
}

#[test]
fn parse_query_run_count_flag() {
    match query_action(&["bzr", "query", "run", "fx", "--count"]) {
        QueryAction::Run(RunArgs { count, .. }) => assert!(count),
        _ => panic!("expected Run"),
    }
}

#[test]
fn parse_query_run_overrides_filters_and_url() {
    match query_action(&[
        "bzr",
        "query",
        "run",
        "fx",
        "--whiteboard",
        "needs-triage",
        "--url",
        "http://crash",
        "--url",
        "http://hang",
    ]) {
        QueryAction::Run(RunArgs { filters, url, .. }) => {
            assert_eq!(filters.whiteboard, vec!["needs-triage"]);
            assert_eq!(url, vec!["http://crash", "http://hang"]);
        }
        _ => panic!("expected Run"),
    }
}

#[test]
fn parse_query_run_date_overrides() {
    match query_action(&[
        "bzr",
        "query",
        "run",
        "fx",
        "--created-since",
        "2026-01-01",
        "--changed-since",
        "2026-05-01",
    ]) {
        QueryAction::Run(RunArgs {
            created_since,
            changed_since,
            ..
        }) => {
            assert_eq!(created_since.as_deref(), Some("2026-01-01"));
            assert_eq!(changed_since.as_deref(), Some("2026-05-01"));
        }
        _ => panic!("expected Run"),
    }
}

#[test]
fn parse_query_run_rejects_offset_with_paginate() {
    assert_eq!(
        parse_error_kind(&["bzr", "query", "run", "fx", "--offset", "10", "--paginate"]),
        ErrorKind::ArgumentConflict
    );
}

// ---- QueryRunFilterArgs::overrides() field-wiring tests ----
// Each test sets exactly one filter flag and asserts the corresponding
// Overrides field is Some(values), not None. A `delete field X` mutant
// would leave that field at its Default (None) and fail the assertion.

use super::QueryRunFilterArgs;
use crate::types::bug::Overrides;

fn filter_args_with(f: impl FnOnce(&mut QueryRunFilterArgs)) -> QueryRunFilterArgs {
    let mut a = QueryRunFilterArgs::default();
    f(&mut a);
    a
}

#[test]
fn overrides_target_milestone_is_some_when_set() {
    let args = filter_args_with(|a| a.target_milestone = vec!["5.0".into()]);
    let url: Vec<String> = vec![];
    let ov: Overrides<'_> = args.overrides(&url);
    assert_eq!(
        ov.target_milestone,
        Some(["5.0".to_string()].as_slice()),
        "target_milestone field must be wired through overrides()"
    );
}

#[test]
fn overrides_version_is_some_when_set() {
    let args = filter_args_with(|a| a.version = vec!["9.4".into()]);
    let url: Vec<String> = vec![];
    let ov: Overrides<'_> = args.overrides(&url);
    assert_eq!(
        ov.version,
        Some(["9.4".to_string()].as_slice()),
        "version field must be wired through overrides()"
    );
}

#[test]
fn overrides_op_sys_is_some_when_set() {
    let args = filter_args_with(|a| a.op_sys = vec!["Linux".into()]);
    let url: Vec<String> = vec![];
    let ov: Overrides<'_> = args.overrides(&url);
    assert_eq!(
        ov.op_sys,
        Some(["Linux".to_string()].as_slice()),
        "op_sys field must be wired through overrides()"
    );
}

#[test]
fn overrides_platform_is_some_when_set() {
    let args = filter_args_with(|a| a.platform = vec!["x86_64".into()]);
    let url: Vec<String> = vec![];
    let ov: Overrides<'_> = args.overrides(&url);
    assert_eq!(
        ov.platform,
        Some(["x86_64".to_string()].as_slice()),
        "platform field must be wired through overrides()"
    );
}

#[test]
fn overrides_qa_contact_is_some_when_set() {
    let args = filter_args_with(|a| a.qa_contact = vec!["qa@example.com".into()]);
    let url: Vec<String> = vec![];
    let ov: Overrides<'_> = args.overrides(&url);
    assert_eq!(
        ov.qa_contact,
        Some(["qa@example.com".to_string()].as_slice()),
        "qa_contact field must be wired through overrides()"
    );
}

#[test]
fn overrides_url_is_some_when_set() {
    let args = QueryRunFilterArgs::default();
    let url = vec!["http://crash.example.com".into()];
    let ov: Overrides<'_> = args.overrides(&url);
    assert_eq!(
        ov.url,
        Some(["http://crash.example.com".to_string()].as_slice()),
        "url field must be wired through overrides()"
    );
}

#[test]
fn overrides_none_when_field_empty() {
    // Baseline: empty vecs produce None — confirms the real behavior is conditional.
    let args = QueryRunFilterArgs::default();
    let url: Vec<String> = vec![];
    let ov: Overrides<'_> = args.overrides(&url);
    assert!(ov.target_milestone.is_none());
    assert!(ov.version.is_none());
    assert!(ov.op_sys.is_none());
    assert!(ov.platform.is_none());
    assert!(ov.qa_contact.is_none());
    assert!(ov.url.is_none());
}
