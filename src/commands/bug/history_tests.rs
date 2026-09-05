#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use super::flatten_history;
use crate::cli::BugAction;
use crate::test_helpers::setup_test_env;
use crate::types::{Comment, FieldChange, HistoryEntry, OutputFormat};

fn entry(who: &str, when: &str, changes: Vec<(&str, &str, &str)>) -> HistoryEntry {
    HistoryEntry {
        who: who.to_string(),
        when: when.to_string(),
        changes: changes
            .into_iter()
            .map(|(field, removed, added)| FieldChange {
                field_name: field.to_string(),
                removed: Some(removed.to_string()),
                added: Some(added.to_string()),
                attachment_id: None,
            })
            .collect(),
    }
}

fn comment(id: u64, creator: &str, creation_time: &str) -> Comment {
    Comment {
        id,
        bug_id: Some(1),
        text: Some(String::new()),
        creator: Some(creator.to_string()),
        creation_time: Some(creation_time.to_string()),
        count: Some(id),
        is_private: Some(false),
        attachment_id: None,
        tags: vec![],
    }
}

#[test]
fn flatten_expands_multi_field_entry_to_one_record_per_field() {
    let entries = vec![entry(
        "alice@example.com",
        "2026-06-01T14:22:01Z",
        vec![("status", "NEW", "ASSIGNED"), ("assigned_to", "", "alice")],
    )];

    let records = flatten_history(&entries, &[]);

    assert_eq!(records.len(), 2, "two changes → two records");
    for r in &records {
        assert_eq!(r.who, "alice@example.com");
        assert_eq!(r.when, "2026-06-01T14:22:01Z");
        assert_eq!(r.comment_id, None);
    }
    assert_eq!(records[0].field, "status");
    assert_eq!(records[0].old_value.as_deref(), Some("NEW"));
    assert_eq!(records[0].new_value.as_deref(), Some("ASSIGNED"));
    assert_eq!(records[1].field, "assigned_to");
    assert_eq!(records[1].old_value.as_deref(), Some(""));
    assert_eq!(records[1].new_value.as_deref(), Some("alice"));
}

#[test]
fn flatten_preserves_missing_delta_values_as_unknown() {
    let entries = vec![HistoryEntry {
        who: "alice@example.com".into(),
        when: "2026-06-01T14:22:01Z".into(),
        changes: vec![FieldChange {
            field_name: "status".into(),
            removed: None,
            added: None,
            attachment_id: None,
        }],
    }];

    let records = flatten_history(&entries, &[]);

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].old_value, None);
    assert_eq!(records[0].new_value, None);
}

#[test]
fn flatten_correlates_comment_id_on_who_and_when_match() {
    let entries = vec![entry(
        "alice@example.com",
        "2026-06-01T14:22:01Z",
        vec![("status", "NEW", "ASSIGNED")],
    )];
    let comments = vec![comment(7, "alice@example.com", "2026-06-01T14:22:01Z")];

    let records = flatten_history(&entries, &comments);

    assert_eq!(records[0].comment_id, Some(7));
}

#[test]
fn flatten_correlates_across_timestamp_forms() {
    // History uses the `Z` form; the comment uses the naive form. Both reduce to
    // the same canonical key, so they still correlate.
    let entries = vec![entry(
        "alice@example.com",
        "2026-06-01T14:22:01Z",
        vec![("status", "NEW", "ASSIGNED")],
    )];
    let comments = vec![comment(7, "alice@example.com", "2026-06-01T14:22:01")];

    let records = flatten_history(&entries, &comments);

    assert_eq!(records[0].comment_id, Some(7));
}

#[test]
fn flatten_no_correlation_when_who_differs() {
    let entries = vec![entry(
        "alice@example.com",
        "2026-06-01T14:22:01Z",
        vec![("status", "NEW", "ASSIGNED")],
    )];
    let comments = vec![comment(7, "bob@example.com", "2026-06-01T14:22:01Z")];

    let records = flatten_history(&entries, &comments);

    assert_eq!(records[0].comment_id, None, "different author → no match");
}

#[test]
fn flatten_no_correlation_when_when_differs() {
    let entries = vec![entry(
        "alice@example.com",
        "2026-06-01T14:22:01Z",
        vec![("status", "NEW", "ASSIGNED")],
    )];
    let comments = vec![comment(7, "alice@example.com", "2026-06-02T09:00:00Z")];

    let records = flatten_history(&entries, &comments);

    assert_eq!(records[0].comment_id, None, "different instant → no match");
}

#[test]
fn flatten_duplicate_keys_resolve_to_smallest_comment_id() {
    let entries = vec![entry(
        "alice@example.com",
        "2026-06-01T14:22:01Z",
        vec![("status", "NEW", "ASSIGNED")],
    )];
    // Two comments by the same author at the same instant; lowest id wins.
    let comments = vec![
        comment(9, "alice@example.com", "2026-06-01T14:22:01Z"),
        comment(4, "alice@example.com", "2026-06-01T14:22:01Z"),
    ];

    let records = flatten_history(&entries, &comments);

    assert_eq!(records[0].comment_id, Some(4));
}

#[test]
fn flatten_skips_unkeyable_and_partial_comments() {
    let entries = vec![entry(
        "alice@example.com",
        "2026-06-01T14:22:01Z",
        vec![("status", "NEW", "ASSIGNED")],
    )];
    // creation_time bears a numeric offset → timestamp_compare_key returns None,
    // so this comment cannot be indexed and the record stays null.
    let comments = vec![comment(7, "alice@example.com", "2026-06-01T14:22:01+01:00")];

    let records = flatten_history(&entries, &comments);

    assert_eq!(records[0].comment_id, None);
}

#[test]
fn flatten_empty_entries_yield_no_records() {
    assert!(flatten_history(&[], &[]).is_empty());
}

#[tokio::test]
async fn bug_history_empty_prints_no_history_message() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/history"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{ "id": 42, "history": [] }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::History(crate::cli::HistoryArgs {
        id: 42,
        since: None,
    });
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None),
        &mut __io.writers(),
    )
    .await;
    let output = __io.out_str().to_string();
    assert!(result.is_ok(), "bug history should succeed: {result:?}");
    assert!(
        output.contains("No history for bug #42."),
        "expected empty-history message, got: {output}"
    );
}

/// Drive `bug history` end to end and return (stdout, stderr, ok).
async fn run_history(id: u64, format: OutputFormat) -> (String, String, bool) {
    let action = BugAction::History(crate::cli::HistoryArgs { id, since: None });
    let mut io = crate::test_helpers::CapturedIo::new();
    let ok = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, format, None),
        &mut io.writers(),
    )
    .await
    .is_ok();
    (io.out_str().to_string(), io.err_str().to_string(), ok)
}

fn history_mock() -> wiremock::Mock {
    Mock::given(method("GET"))
        .and(path("/rest/bug/7/history"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 7,
                "history": [{
                    "who": "alice@example.com",
                    "when": "2026-06-01T14:22:01Z",
                    "changes": [
                        {"field_name": "status", "removed": "NEW", "added": "ASSIGNED"},
                        {"field_name": "assigned_to", "removed": "", "added": "alice@example.com"}
                    ]
                }]
            }]
        })))
}

#[tokio::test]
async fn bug_history_json_expands_multi_field_entry_to_multiple_records() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    history_mock().expect(1).mount(&mock).await;
    // A comment at the same who+when correlates a comment_id onto both records.
    Mock::given(method("GET"))
        .and(path("/rest/bug/7/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": { "7": { "comments": [{
                "id": 42, "bug_id": 7, "text": "moving along",
                "creator": "alice@example.com",
                "creation_time": "2026-06-01T14:22:01Z",
                "is_private": false, "count": 1
            }] } }
        })))
        .mount(&mock)
        .await;

    let (out, _err, ok) = run_history(7, OutputFormat::Json).await;
    assert!(ok, "bug history --json should succeed");
    let data = crate::test_helpers::json_envelope_data(&out);
    let arr = data.as_array().unwrap();
    assert_eq!(arr.len(), 2, "two changed fields → two records: {out}");
    assert_eq!(arr[0]["field"], "status");
    assert_eq!(arr[0]["old_value"], "NEW");
    assert_eq!(arr[0]["new_value"], "ASSIGNED");
    assert_eq!(arr[1]["field"], "assigned_to");
    // when/who/comment_id shared across the expanded records.
    for rec in arr {
        assert_eq!(rec["when"], "2026-06-01T14:22:01Z");
        assert_eq!(rec["who"], "alice@example.com");
        assert_eq!(rec["comment_id"], 42);
    }
}

#[tokio::test]
async fn bug_history_ndjson_streams_one_record_per_line() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    history_mock().mount(&mock).await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/7/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": { "7": { "comments": [] } }
        })))
        .mount(&mock)
        .await;

    let (out, _err, ok) = run_history(7, OutputFormat::Ndjson).await;
    assert!(ok);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "one record per changed field: {out}");
    for line in lines {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v["field"].is_string());
        // No comment at this instant → null, but the key is present.
        assert!(v.get("comment_id").is_some());
        assert!(v["comment_id"].is_null());
    }
}

#[tokio::test]
async fn bug_history_json_empty_emits_empty_array_not_prose() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/7/history"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{ "id": 7, "history": [] }]
        })))
        .mount(&mock)
        .await;
    // Empty history must NOT trigger the comment correlation fetch.
    Mock::given(method("GET"))
        .and(path("/rest/bug/7/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": { "7": { "comments": [] } }
        })))
        .expect(0)
        .mount(&mock)
        .await;

    let (out, _err, ok) = run_history(7, OutputFormat::Json).await;
    assert!(ok);
    assert!(
        !out.contains("No history"),
        "JSON output must be machine-readable, not prose: {out}"
    );
    let data = crate::test_helpers::json_envelope_data(&out);
    assert_eq!(data, serde_json::json!([]));
}

#[tokio::test]
async fn bug_history_json_degrades_to_null_comment_id_when_comment_fetch_fails() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    history_mock().mount(&mock).await;
    // Comment fetch fails: correlation degrades to null, command still succeeds.
    Mock::given(method("GET"))
        .and(path("/rest/bug/7/comment"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&mock)
        .await;

    let (out, err, ok) = run_history(7, OutputFormat::Json).await;
    assert!(
        ok,
        "history delta is the contract; comment failure is non-fatal"
    );
    let data = crate::test_helpers::json_envelope_data(&out);
    let arr = data.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    for rec in arr {
        assert!(rec["comment_id"].is_null());
    }
    assert!(
        err.contains("comment_id will be null"),
        "a warning should explain the degradation: {err}"
    );
}

#[tokio::test]
async fn bug_history_rejects_malformed_since_with_exit_code_7() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = BugAction::History(crate::cli::HistoryArgs {
        id: 42,
        since: Some("yesterday".into()),
    });
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None),
        &mut __cap_io.writers(),
    )
    .await;
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 7);
    let msg = err.to_string();
    assert!(msg.contains("--since"), "error should name the flag: {msg}");
    assert!(
        msg.contains("yesterday"),
        "error should echo the offending input: {msg}"
    );
}
