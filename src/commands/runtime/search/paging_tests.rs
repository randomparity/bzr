#![expect(clippy::panic, clippy::unwrap_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{
    fetch_all_pages_with_cap, fetch_page, resolve_page_window, write_truncation_note, Page,
};
use crate::client::test_helpers::{test_client, test_client_hybrid};
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

fn raw_params_with_limit(limit: u32) -> SearchParams {
    SearchParams {
        limit: Some(limit),
        include_fields: Some("summary".into()),
        exclude_fields: Some("id".into()),
        raw_params: vec![
            ("f1".into(), "status_whiteboard".into()),
            ("o1".into(), "substring".into()),
            ("v1".into(), "marker".into()),
        ],
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
    let page = fetch_page(
        &client,
        &params_with_limit(2),
        false,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
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
async fn fetch_page_raw_params_rejects_limit_that_cannot_overfetch() {
    let mock = MockServer::start().await;
    let client = test_client_hybrid(&mock.uri());
    let (capture, _guard) = crate::test_helpers::TracingCapture::install(tracing::Level::WARN);

    let Err(err) = fetch_page(
        &client,
        &raw_params_with_limit(u32::MAX),
        false,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    else {
        panic!("expected limit overflow validation");
    };

    assert!(
        matches!(&err, crate::error::BzrError::InputValidation { message: msg, .. }
            if msg.contains("--limit") && msg.contains("too large")),
        "limit overflow should be rejected before search: {err:?}"
    );
    assert!(
        mock.received_requests().await.unwrap().is_empty(),
        "overflow validation should happen before any search request"
    );
    assert!(
        !capture
            .output()
            .contains("query contains raw URL parameters that require REST API"),
        "overflow validation should happen before the fallback warning"
    );
}

#[tokio::test]
async fn fetch_page_rejects_zero_limit_with_nonzero_offset_before_search() {
    let mock = MockServer::start().await;
    let client = test_client(&mock.uri());
    let params = SearchParams {
        limit: Some(0),
        offset: Some(1),
        ..Default::default()
    };

    let result = fetch_page(
        &client,
        &params,
        false,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await;
    let Err(err) = result else {
        panic!("expected zero-limit paging validation");
    };

    assert!(
        matches!(&err, crate::error::BzrError::InputValidation { message, .. }
        if message.contains("--limit 0") && message.contains("nonzero --offset"))
    );
    assert!(mock.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn fetch_page_allows_zero_limit_with_zero_offset() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("limit", "0"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(1)))
        .expect(1)
        .mount(&mock)
        .await;
    let client = test_client(&mock.uri());
    let params = SearchParams {
        limit: Some(0),
        offset: Some(0),
        ..Default::default()
    };

    let page = fetch_page(
        &client,
        &params,
        false,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    assert_eq!(page.bugs.len(), 1);
}

#[tokio::test]
async fn fetch_all_pages_raw_params_warns_once_and_uses_rest() {
    let mock = MockServer::start().await;
    for (offset, body) in [
        ("0", serde_json::json!({"bugs": [{"id": 1}, {"id": 2}]})),
        ("2", serde_json::json!({"bugs": [{"id": 3}]})),
        ("3", serde_json::json!({"bugs": []})),
    ] {
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("limit", "2"))
            .and(query_param("offset", offset))
            .and(query_param("f1", "status_whiteboard"))
            .and(query_param("o1", "substring"))
            .and(query_param("v1", "marker"))
            .and(query_param("include_fields", "id,summary"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .expect(1)
            .mount(&mock)
            .await;
    }

    let client = test_client_hybrid(&mock.uri());
    let (capture, _guard) = crate::test_helpers::TracingCapture::install(tracing::Level::WARN);
    let bugs = fetch_all_pages_with_cap(
        &client,
        &raw_params_with_limit(2),
        3,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    assert_eq!(
        bugs.iter().map(|bug| bug.id).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    let requests = mock.received_requests().await.unwrap();
    assert_eq!(requests.len(), 3);
    for request in requests {
        assert_eq!(request.method.as_str(), "GET");
        assert_eq!(request.url.path(), "/rest/bug");
        assert!(
            !request
                .url
                .query_pairs()
                .any(|(key, _)| key == "exclude_fields"),
            "id must be removed from exclude_fields"
        );
    }
    assert_eq!(
        capture
            .output()
            .matches("query contains raw URL parameters that require REST API")
            .count(),
        1
    );
}

#[tokio::test]
async fn fetch_page_raw_params_explicit_rest_is_silent() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("f1", "status_whiteboard"))
        .and(query_param("o1", "substring"))
        .and(query_param("v1", "marker"))
        .and(query_param("include_fields", "id,summary"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(1)))
        .expect(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let (capture, _guard) = crate::test_helpers::TracingCapture::install(tracing::Level::WARN);
    let params = raw_params_with_limit(0);
    let page = fetch_page(
        &client,
        &params,
        true,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    assert_eq!(page.bugs.len(), 1);
    assert!(!capture
        .output()
        .contains("query contains raw URL parameters that require REST API"));
}

#[tokio::test]
async fn fetch_page_rejects_next_offset_overflow() {
    let mock = MockServer::start().await;
    let client = test_client(&mock.uri());
    let params = SearchParams {
        limit: Some(10),
        offset: Some(u32::MAX - 5),
        ..Default::default()
    };

    let Err(err) = fetch_page(
        &client,
        &params,
        false,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    else {
        panic!("expected next offset overflow validation");
    };

    assert!(
        matches!(&err, crate::error::BzrError::InputValidation { message: msg, .. }
            if msg.contains("--offset") && msg.contains("page size")),
        "next offset overflow should be rejected before search: {err:?}"
    );
    assert!(
        mock.received_requests().await.unwrap().is_empty(),
        "overflow validation should happen before any search request"
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
    let page = fetch_page(
        &client,
        &params_with_limit(5),
        false,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    assert!(!page.truncated);
    assert_eq!(page.bugs.len(), 3);
}

#[tokio::test]
async fn fetch_page_paginate_loops_until_empty_page() {
    let mock = MockServer::start().await;
    // Page size 2: the short page still needs an empty terminal request.
    for (off, n) in [("0", 2u64), ("2", 2), ("4", 1), ("5", 0)] {
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("offset", off))
            .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(n)))
            .mount(&mock)
            .await;
    }

    let client = test_client(&mock.uri());
    let page = fetch_page(
        &client,
        &params_with_limit(2),
        true,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    assert!(!page.truncated, "paginate retrieves everything");
    assert_eq!(page.bugs.len(), 5, "2 + 2 + 1 followed by an empty page");
}

#[tokio::test]
async fn fetch_page_paginate_rejects_next_offset_overflow() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(6)))
        .mount(&mock)
        .await;
    let client = test_client(&mock.uri());
    let params = SearchParams {
        limit: Some(10),
        offset: Some(u32::MAX - 5),
        ..Default::default()
    };

    let Err(err) = fetch_page(
        &client,
        &params,
        true,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    else {
        panic!("expected paginate offset overflow validation");
    };

    assert!(
        matches!(&err, crate::error::BzrError::InputValidation { message: msg, .. }
            if msg.contains("--offset") && msg.contains("page size")),
        "paginate offset overflow should be rejected after observing the batch: {err:?}"
    );
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

#[test]
fn resolve_page_window_folds_url_offset_into_field() {
    // A `--from-url` import (or a saved query from one) leaves `offset=` sitting
    // in `raw_params`. The resolver must match that *specific* key, fold its
    // value into the struct field, and strip the raw copy so the request never
    // sends two conflicting `offset` params. Matching "any param except offset"
    // (the `k != "offset"` mutant) would leave the field unset.
    let mut params = SearchParams {
        raw_params: vec![("offset".to_string(), "5".to_string())],
        ..Default::default()
    };

    resolve_page_window(&mut params, None);

    assert_eq!(
        params.offset,
        Some(5),
        "the url's offset=5 must be folded into the structured field"
    );
    assert!(
        !params.raw_params.iter().any(|(k, _)| k == "offset"),
        "the raw offset copy must be stripped to avoid a duplicate query param"
    );
}

#[test]
fn resolve_page_window_cli_overrides_url() {
    // An explicit `--offset` on the command line wins over a url-carried offset.
    let mut params = SearchParams {
        raw_params: vec![("offset".to_string(), "5".to_string())],
        ..Default::default()
    };

    resolve_page_window(&mut params, Some(9));

    assert_eq!(
        params.offset,
        Some(9),
        "explicit --offset 9 overrides url offset=5"
    );
}

#[test]
fn resolve_page_window_removes_raw_limit_without_overriding_typed_limit() {
    let mut params = SearchParams {
        limit: Some(1),
        raw_params: vec![("limit".into(), "0".into()), ("offset".into(), "4".into())],
        ..Default::default()
    };

    resolve_page_window(&mut params, None);

    assert_eq!(params.limit, Some(1));
    assert_eq!(params.offset, Some(4));
    assert!(
        params
            .raw_params
            .iter()
            .all(|(key, _)| key != "limit" && key != "offset"),
        "resolved paging keys must not remain as duplicate raw params"
    );
}

#[tokio::test]
async fn fetch_page_exact_fill_is_not_truncated() {
    let mock = MockServer::start().await;
    // limit 2 → over-fetch requests limit=3, but the server has exactly 2 rows.
    // The window is filled precisely with nothing beyond it: `len > limit` is
    // false. The `>=` mutant would read 2 >= 2 as "more available" and wrongly
    // flag truncation. This is the off-by-one boundary the other tests skip
    // (they return strictly more, or strictly fewer, than the limit).
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("limit", "3"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(2)))
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let page = fetch_page(
        &client,
        &params_with_limit(2),
        false,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    assert!(
        !page.truncated,
        "exactly `limit` rows means the window is full with nothing beyond it"
    );
    assert_eq!(page.bugs.len(), 2);
}

#[tokio::test]
async fn fetch_page_paginate_zero_limit_makes_single_unbounded_request() {
    let mock = MockServer::start().await;
    // A zero limit means "no window": `--paginate` must collapse to one
    // unbounded request rather than entering the offset-stepping loop. The
    // `*l > 0` guard filters the 0 out. The `*l >= 0` mutant (always true for
    // u32) would admit a `page_size` of 0 and step `offset += 0` forever; the
    // bounded mock turns that runaway loop into a hard error on the 2nd request.
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(1)))
        .up_to_n_times(1)
        .mount(&mock)
        .await;

    let client = test_client(&mock.uri());
    let params = SearchParams {
        limit: Some(0),
        ..Default::default()
    };
    let page = fetch_page(
        &client,
        &params,
        true,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    assert_eq!(
        page.bugs.len(),
        1,
        "a single unbounded request returns the one page as-is"
    );
}

#[tokio::test]
async fn fetch_all_pages_errors_when_safety_cap_reached_without_empty_page() {
    let mock = MockServer::start().await;
    for off in ["0", "2"] {
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("offset", off))
            .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(2)))
            .mount(&mock)
            .await;
    }

    let client = test_client(&mock.uri());
    let err = fetch_all_pages_with_cap(
        &client,
        &params_with_limit(2),
        2,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(&err, crate::error::BzrError::InputValidation { message: msg, .. }
            if msg.contains("--paginate") && msg.contains("safety cap")),
        "cap exhaustion should fail instead of returning incomplete results: {err:?}"
    );
}

#[tokio::test]
async fn fetch_all_pages_advances_by_server_clamped_batch_size() {
    let mock = MockServer::start().await;
    for (off, body) in [
        ("0", serde_json::json!({"bugs": [{"id": 1}, {"id": 2}]})),
        ("2", serde_json::json!({"bugs": [{"id": 3}, {"id": 4}]})),
        ("4", serde_json::json!({"bugs": [{"id": 5}]})),
        ("5", serde_json::json!({"bugs": []})),
    ] {
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("limit", "100"))
            .and(query_param("offset", off))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&mock)
            .await;
    }

    let client = test_client(&mock.uri());
    let bugs = fetch_all_pages_with_cap(
        &client,
        &params_with_limit(100),
        4,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    assert_eq!(
        bugs.iter().map(|bug| bug.id).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5],
        "clamped pages must be contiguous, complete, and free of duplicates"
    );
}

#[tokio::test]
async fn fetch_page_paginate_emits_page_events_but_not_done() {
    // `fetch_page` emits one `page` per request during the loop; the terminal
    // `done` is the caller's responsibility (emitted only after the whole
    // invocation succeeds), so it must NOT appear in the fetch's own output.
    let mock = MockServer::start().await;
    for (off, n) in [("0", 2u64), ("2", 2), ("4", 1), ("5", 0)] {
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param("offset", off))
            .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(n)))
            .mount(&mock)
            .await;
    }
    let client = test_client(&mock.uri());
    let mut io = crate::test_helpers::CapturedIo::new();
    let page = fetch_page(
        &client,
        &params_with_limit(2),
        true,
        Some(crate::types::ProgressFormat::Ndjson),
        &mut io.writers(),
    )
    .await
    .unwrap();
    assert_eq!(page.bugs.len(), 5);
    assert!(io.out_str().is_empty(), "progress must not touch stdout");
    assert_eq!(
        io.err_str().lines().collect::<Vec<_>>(),
        vec![
            "{\"event\":\"page\",\"n\":1,\"fetched\":2}",
            "{\"event\":\"page\",\"n\":2,\"fetched\":4}",
            "{\"event\":\"page\",\"n\":3,\"fetched\":5}",
            "{\"event\":\"page\",\"n\":4,\"fetched\":5}",
        ],
        "no terminal done from fetch_page itself"
    );
}

#[tokio::test]
async fn fetch_page_paginate_zero_result_emits_page_not_done() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(0)))
        .mount(&mock)
        .await;
    let client = test_client(&mock.uri());
    let mut io = crate::test_helpers::CapturedIo::new();
    let page = fetch_page(
        &client,
        &params_with_limit(2),
        true,
        Some(crate::types::ProgressFormat::Ndjson),
        &mut io.writers(),
    )
    .await
    .unwrap();
    assert!(page.bugs.is_empty());
    assert_eq!(
        io.err_str().lines().collect::<Vec<_>>(),
        vec!["{\"event\":\"page\",\"n\":1,\"fetched\":0}"],
        "a zero-result paginate reports its (empty) page but defers done to the caller"
    );
}

#[tokio::test]
async fn fetch_page_paginate_unbounded_emits_nothing_from_fetch() {
    // A zero/absent limit collapses to one unbounded request: no page sequence,
    // and the terminal `done` is the caller's, so fetch_page emits nothing.
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(3)))
        .up_to_n_times(1)
        .mount(&mock)
        .await;
    let client = test_client(&mock.uri());
    let params = SearchParams {
        limit: Some(0),
        ..Default::default()
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let page = fetch_page(
        &client,
        &params,
        true,
        Some(crate::types::ProgressFormat::Ndjson),
        &mut io.writers(),
    )
    .await
    .unwrap();
    assert_eq!(page.bugs.len(), 3);
    assert!(
        io.err_str().is_empty(),
        "unbounded fetch emits no progress; the caller emits the terminal done"
    );
}

#[tokio::test]
async fn fetch_page_none_progress_is_silent_on_stderr() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("offset", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(1)))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("offset", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(0)))
        .mount(&mock)
        .await;
    let client = test_client(&mock.uri());
    let mut io = crate::test_helpers::CapturedIo::new();
    let _ = fetch_page(
        &client,
        &params_with_limit(2),
        true,
        None,
        &mut io.writers(),
    )
    .await
    .unwrap();
    assert!(io.err_str().is_empty(), "None progress emits nothing");
}

#[tokio::test]
async fn paginate_stdout_identical_with_and_without_progress() {
    async fn run(progress: Option<crate::types::ProgressFormat>) -> Vec<u8> {
        let mock = MockServer::start().await;
        for (off, n) in [("0", 2u64), ("2", 1), ("3", 0)] {
            Mock::given(method("GET"))
                .and(path("/rest/bug"))
                .and(query_param("offset", off))
                .respond_with(ResponseTemplate::new(200).set_body_json(bugs_body(n)))
                .mount(&mock)
                .await;
        }
        let client = test_client(&mock.uri());
        let mut io = crate::test_helpers::CapturedIo::new();
        let page = fetch_page(
            &client,
            &params_with_limit(2),
            true,
            progress,
            &mut io.writers(),
        )
        .await
        .unwrap();
        // Emulate the caller writing the result document to stdout.
        let _ = std::io::Write::write_all(&mut io.out, format!("{}\n", page.bugs.len()).as_bytes());
        io.out
    }
    assert_eq!(
        run(None).await,
        run(Some(crate::types::ProgressFormat::Ndjson)).await,
        "stdout must be byte-identical regardless of --progress"
    );
}
