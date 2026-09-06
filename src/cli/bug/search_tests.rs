#![expect(clippy::unwrap_used, clippy::panic)]

use super::SearchArgs;
use crate::cli::bug::BugAction;
use crate::cli::{Cli, Commands};
use clap::error::ErrorKind;
use clap::Parser as _;

fn search_args(args: &[&str]) -> SearchArgs {
    match Cli::try_parse_from(args).unwrap().command {
        Commands::Bug {
            action: BugAction::Search(search),
        } => search,
        _ => panic!("expected Commands::Bug {{ action: Search }}"),
    }
}

/// Parse arguments expected to fail and return the clap error kind, so
/// negative tests pin *why* parsing was rejected rather than accepting any
/// failure (which would mask argv drift in the test itself).
fn parse_error_kind(args: &[&str]) -> ErrorKind {
    Cli::try_parse_from(args).err().unwrap().kind()
}

#[test]
fn parse_search_quicksearch_query() {
    let search = search_args(&["bzr", "bug", "search", "kernel panic", "--limit", "10"]);
    assert_eq!(search.query.as_deref(), Some("kernel panic"));
    assert!(search.from_url.is_none());
    assert_eq!(search.limit, Some(10));
    assert!(!search.count);
}

#[test]
fn parse_search_count_and_fields() {
    let search = search_args(&[
        "bzr",
        "bug",
        "search",
        "crash",
        "--count",
        "--fields",
        "id,summary",
    ]);
    assert!(search.count);
    assert_eq!(search.field_args.fields.as_deref(), Some("id,summary"));
}

#[test]
fn parse_search_from_url() {
    let search = search_args(&[
        "bzr",
        "bug",
        "search",
        "--from-url",
        "https://bz/buglist.cgi?product=Firefox",
    ]);
    assert_eq!(
        search.from_url.as_deref(),
        Some("https://bz/buglist.cgi?product=Firefox")
    );
    assert!(search.query.is_none());
}

#[test]
fn parse_search_from_url_with_save_as_named() {
    let search = search_args(&[
        "bzr",
        "bug",
        "search",
        "--from-url",
        "https://bz/buglist.cgi?product=Firefox",
        "--save-as",
        "firefox-bugs",
    ]);
    assert_eq!(search.save_as.as_deref(), Some("firefox-bugs"));
}

#[test]
fn parse_search_save_as_without_value_defaults_to_empty() {
    // `--save-as` with no value resolves to the URL's known_name at runtime;
    // at the parser layer the missing value becomes an empty string.
    let search = search_args(&[
        "bzr",
        "bug",
        "search",
        "--from-url",
        "https://bz/buglist.cgi?known_name=fx",
        "--save-as",
    ]);
    assert_eq!(search.save_as.as_deref(), Some(""));
}

#[test]
fn search_rejects_query_with_from_url() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "bug",
            "search",
            "kernel panic",
            "--from-url",
            "https://bz/buglist.cgi",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn search_save_as_requires_from_url() {
    // `--save-as` declares `requires = "from_url"`. With no quicksearch
    // positional present, clap enforces that relationship at parse time. (When
    // a positional query is also supplied the requirement is only caught at
    // runtime, exit 7 -- so this asserts the clap-layer enforcement path.)
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "search", "--save-as", "name"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn saved_search_conflicts_with_positional_query() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "search", "crash", "--saved-search", "triage"]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn saved_search_conflicts_with_from_url() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "bug",
            "search",
            "--from-url",
            "https://bz.example/buglist.cgi?product=X",
            "--saved-search",
            "triage",
        ]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn sharer_requires_saved_search() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "search", "--sharer", "1"]),
        ErrorKind::MissingRequiredArgument
    );
}

#[test]
fn sharer_rejects_non_numeric_id() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "bug",
            "search",
            "--saved-search",
            "triage",
            "--sharer",
            "not-a-number",
        ]),
        ErrorKind::ValueValidation
    );
}

#[test]
fn parse_saved_search_with_sharer() {
    let search = search_args(&[
        "bzr",
        "bug",
        "search",
        "--saved-search",
        "team list",
        "--sharer",
        "112233",
    ]);
    assert_eq!(search.saved_search.as_deref(), Some("team list"));
    assert_eq!(search.sharer, Some(112_233));
    assert!(search.query.is_none());
}

/// Without matching conflicts on `--sharer`, clap suppresses its `requires`
/// check when a positional query is present and silently ignores the flag.
#[test]
fn sharer_with_positional_query_is_rejected_not_ignored() {
    assert_eq!(
        parse_error_kind(&["bzr", "bug", "search", "crash", "--sharer", "1"]),
        ErrorKind::ArgumentConflict
    );
}

#[test]
fn sharer_with_from_url_is_rejected_not_ignored() {
    assert_eq!(
        parse_error_kind(&[
            "bzr",
            "bug",
            "search",
            "--from-url",
            "https://bz.example/buglist.cgi?product=X",
            "--sharer",
            "1",
        ]),
        ErrorKind::ArgumentConflict
    );
}
