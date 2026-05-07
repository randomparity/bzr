#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::BugAction;
use crate::test_helpers::{capture_stdout, setup_test_env};
use crate::types::OutputFormat;

fn empty_list_action() -> BugAction {
    BugAction::List {
        product: vec![],
        component: vec![],
        status: vec![],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        id: vec![],
        alias: None,
        summary: None,
        limit: 50,
        fields: None,
        exclude_fields: None,
        created_since: None,
        changed_since: None,
    }
}

#[tokio::test]
async fn bug_list_returns_bugs() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 1,
                "summary": "Test bug",
                "status": "NEW",
                "resolution": "",
                "assigned_to": "nobody@test.com",
                "priority": "P1",
                "severity": "normal",
                "product": "TestProduct",
                "component": "General",
                "creation_time": "2025-01-01T00:00:00Z",
                "last_change_time": "2025-01-01T00:00:00Z"
            }]
        })))
        .mount(&mock)
        .await;

    let action = empty_list_action();
    let (result, output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed[0]["id"], 1);
    assert_eq!(parsed[0]["summary"], "Test bug");
    assert_eq!(parsed[0]["status"], "NEW");
    assert_eq!(parsed[0]["product"], "TestProduct");
}

#[tokio::test]
async fn bug_list_passes_every_field_through_to_search_params() {
    // Every CLI field on `bug list` must round-trip into the search
    // query string.
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("product", "Firefox"))
        .and(query_param("component", "General"))
        .and(query_param("status", "NEW"))
        .and(query_param("assigned_to", "dev@test.com"))
        .and(query_param("creator", "reporter@test.com"))
        .and(query_param("priority", "P1"))
        .and(query_param("severity", "major"))
        .and(query_param("id", "42"))
        .and(query_param("alias", "my-alias"))
        .and(query_param("summary", "kernel panic"))
        .and(query_param("limit", "5"))
        .and(query_param("include_fields", "id,summary"))
        .and(query_param("exclude_fields", "comments"))
        .and(query_param("creation_time", "2026-04-01T00:00:00Z"))
        .and(query_param("last_change_time", "2026-04-15T00:00:00Z"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::List {
        product: vec!["Firefox".into()],
        component: vec!["General".into()],
        status: vec!["NEW".into()],
        assignee: vec!["dev@test.com".into()],
        creator: vec!["reporter@test.com".into()],
        priority: vec!["P1".into()],
        severity: vec!["major".into()],
        id: vec![42],
        alias: Some("my-alias".into()),
        summary: Some("kernel panic".into()),
        limit: 5,
        fields: Some("id,summary".into()),
        exclude_fields: Some("comments".into()),
        created_since: Some("2026-04-01".into()),
        changed_since: Some("2026-04-15T00:00:00Z".into()),
    };
    let result = crate::commands::bug::execute(&action, None, OutputFormat::Json, None).await;
    assert!(
        result.is_ok(),
        "bug list with all fields failed: {result:?}"
    );
}

#[tokio::test]
async fn bug_list_summary_only_sends_substring_filter() {
    // `--summary` alone must be passed verbatim as the REST `summary`
    // query parameter and must not trigger an XML-RPC fallback even
    // when the result is empty (issue #152).
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("summary", "WARNING CPU default_machine_kexec"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let action = BugAction::List {
        product: vec![],
        component: vec![],
        status: vec![],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        id: vec![],
        alias: None,
        summary: Some("WARNING CPU default_machine_kexec".into()),
        limit: 50,
        fields: None,
        exclude_fields: None,
        created_since: None,
        changed_since: None,
    };
    let result = crate::commands::bug::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_ok(), "bug list --summary failed: {result:?}");
}

#[tokio::test]
async fn bug_list_http_500_returns_error() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = empty_list_action();
    let result = crate::commands::bug::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("500") || err.contains("Internal Server Error"),
        "expected HTTP 500 error, got: {err}"
    );
}

#[tokio::test]
async fn bug_list_malformed_json_returns_error() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
        .mount(&mock)
        .await;

    let action = empty_list_action();
    let result = crate::commands::bug::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn bug_list_rejects_malformed_created_since_with_exit_code_7() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let mut action = empty_list_action();
    if let BugAction::List { created_since, .. } = &mut action {
        *created_since = Some("not-a-date".into());
    }

    let result = crate::commands::bug::execute(&action, None, OutputFormat::Json, None).await;
    let err = result.unwrap_err();
    assert_eq!(
        err.exit_code(),
        7,
        "expected exit code 7 for input validation, got {err:?}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("--created-since"),
        "error should name the flag: {msg}"
    );
    assert!(
        msg.contains("not-a-date"),
        "error should echo the offending input: {msg}"
    );
}

#[tokio::test]
async fn bug_list_rejects_malformed_changed_since_with_exit_code_7() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let mut action = empty_list_action();
    if let BugAction::List { changed_since, .. } = &mut action {
        *changed_since = Some("2026-13-99".into());
    }

    let result = crate::commands::bug::execute(&action, None, OutputFormat::Json, None).await;
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(err.to_string().contains("--changed-since"));
}
