#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{fetch_page, write_truncation_note, Page};
use crate::client::test_helpers::test_client;
use crate::types::{Bug, OutputFormat, SearchParams};

/// A `Page` of `n` placeholder bugs marked truncated/not for note tests.
fn page_of(n: usize, truncated: bool) -> Page {
    let bugs: Vec<Bug> = (1..=n as u64)
        .map(|id| serde_json::from_value(serde_json::json!({"id": id})).unwrap())
        .collect();
    Page { bugs, truncated }
}

/// A `{"bugs":[...]}` body with `n` bugs (ids 1..=n).
fn bugs_body(n: u64) -> serde_json::Value {
    let bugs: Vec<serde_json::Value> = (1..=n).map(|id| serde_json::json!({"id": id})).collect();
    serde_json::json!({ "bugs": bugs })
}

fn params_with_limit(limit: u32) -> SearchParams {
    SearchParams {
        limit: Some(limit),
        ..Default::default()
    }
}

#[tokio::test]
async fn fetch_page_flags_truncation_via_over_fetch() {
    let mock = MockServer::start().await;
    // limit 2 → over-fetch requests limit=3; server returns 3 → truncated.
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("limit", "3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(3)))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let page = fetch_page(&client, &params_with_limit(2), false)
        .await
        .unwrap();

    assert!(
        page.truncated,
        "3 returned for a limit of 2 means more exist"
    );
    assert_eq!(
        page.bugs.len(),
        2,
        "the over-fetched surplus row is trimmed"
    );
}

#[tokio::test]
async fn fetch_page_no_truncation_when_under_limit() {
    let mock = MockServer::start().await;
    // limit 5 → over-fetch requests limit=6; server returns 3 (< 6).
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("limit", "6"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(3)))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let page = fetch_page(&client, &params_with_limit(5), false)
        .await
        .unwrap();

    assert!(!page.truncated);
    assert_eq!(page.bugs.len(), 3);
}

#[tokio::test]
async fn fetch_page_paginate_loops_until_short_page() {
    let mock = MockServer::start().await;
    // Page size 2: offset 0 → 2 bugs, offset 2 → 2 bugs, offset 4 → 1 (short, stop).
    for (off, n) in [("0", 2u64), ("2", 2), ("4", 1)] {
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("offset", off))
            .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(n)))
            .mount(&mock)
            .await;
    }

    let client = test_client(&mock.uri());
    let page = fetch_page(&client, &params_with_limit(2), true)
        .await
        .unwrap();

    assert!(!page.truncated, "paginate retrieves everything");
    assert_eq!(page.bugs.len(), 5, "2 + 2 + 1 across three pages");
}

#[test]
fn truncation_note_table_goes_to_stdout() {
    let mut io = crate::test_helpers::CapturedIo::new();
    write_truncation_note(
        &page_of(50, true),
        Some(50),
        None,
        OutputFormat::Table,
        &mut io.writers(),
    );
    assert!(io.out_str().contains("Showing first 50"));
    assert!(io.out_str().contains("--paginate"));
    assert!(io.out_str().contains("--offset 50"));
    assert!(io.err_str().is_empty());
}

#[test]
fn truncation_note_json_goes_to_stderr() {
    // Under JSON, the note must not pollute the parseable stdout document.
    let mut io = crate::test_helpers::CapturedIo::new();
    write_truncation_note(
        &page_of(25, true),
        Some(25),
        Some(50),
        OutputFormat::Json,
        &mut io.writers(),
    );
    assert!(io.out_str().is_empty(), "stdout stays clean JSON");
    assert!(io.err_str().contains("Showing first 25"));
    assert!(
        io.err_str().contains("--offset 75"),
        "next offset = 50 + 25"
    );
}

#[test]
fn truncation_note_noop_when_not_truncated() {
    let mut io = crate::test_helpers::CapturedIo::new();
    write_truncation_note(
        &page_of(50, false),
        Some(50),
        None,
        OutputFormat::Table,
        &mut io.writers(),
    );
    assert!(io.out_str().is_empty());
    assert!(io.err_str().is_empty());
}
