#![expect(clippy::expect_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::BugAction;
use crate::test_helpers::{capture_stdout, setup_test_env};
use crate::types::OutputFormat;

#[tokio::test]
async fn bug_my_returns_assigned_by_default() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // Mock whoami
    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "dev@test.com",
            "real_name": "Dev User",
            "id": 1
        })))
        .mount(&mock)
        .await;

    // Mock assigned-to search
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 10,
                "summary": "Assigned bug",
                "status": "NEW",
                "assigned_to": "dev@test.com",
                "product": "TestProduct",
                "component": "General"
            }]
        })))
        .mount(&mock)
        .await;

    let action = BugAction::My {
        created: false,
        cc: false,
        all: false,
        status: vec![],
        limit: 50,
        fields: None,
        exclude_fields: None,
    };
    let (result, output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok(), "bug my failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed[0]["id"], 10);
    assert_eq!(parsed[0]["summary"], "Assigned bug");
}

#[tokio::test]
async fn bug_my_passes_status_limit_and_field_filters() {
    // Pin status, limit, fields, exclude_fields through to the search.
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "dev@test.com", "real_name": "Dev User", "id": 1
        })))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("status", "NEW"))
        .and(query_param("limit", "7"))
        .and(query_param("include_fields", "id,summary"))
        .and(query_param("exclude_fields", "comments"))
        .and(query_param("assigned_to", "dev@test.com"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::My {
        created: false,
        cc: false,
        all: false,
        status: vec!["NEW".into()],
        limit: 7,
        fields: Some("id,summary".into()),
        exclude_fields: Some("comments".into()),
    };
    let (result, _) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok(), "bug my with filters failed: {result:?}");
}

#[tokio::test]
async fn bug_my_created_only_runs_creator_search_not_assigned() {
    // `--created` (without `--all`) must search by `creator=`, NOT by
    // `assigned_to=` and NOT by `cc=`. Each search has a strict matcher
    // and an `expect(...)` count.
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "dev@test.com", "real_name": "Dev User", "id": 1
        })))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("creator", "dev@test.com"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::My {
        created: true,
        cc: false,
        all: false,
        status: vec![],
        limit: 50,
        fields: None,
        exclude_fields: None,
    };
    let (result, _) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok(), "bug my --created failed: {result:?}");
}

#[tokio::test]
async fn bug_my_cc_only_runs_cc_search_not_assigned_or_creator() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "dev@test.com", "real_name": "Dev User", "id": 1
        })))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("cc", "dev@test.com"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::My {
        created: false,
        cc: true,
        all: false,
        status: vec![],
        limit: 50,
        fields: None,
        exclude_fields: None,
    };
    let (result, _) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok(), "bug my --cc failed: {result:?}");
}

#[tokio::test]
async fn bug_my_all_deduplicates() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "dev@test.com",
            "real_name": "Dev User",
            "id": 1
        })))
        .mount(&mock)
        .await;

    // All three searches return the same bug — should appear only once
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 42,
                "summary": "Shared bug",
                "status": "NEW",
                "assigned_to": "dev@test.com",
                "product": "TestProduct",
                "component": "General"
            }]
        })))
        .mount(&mock)
        .await;

    let action = BugAction::My {
        created: false,
        cc: false,
        all: true,
        status: vec![],
        limit: 50,
        fields: None,
        exclude_fields: None,
    };
    let (result, output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok(), "bug my --all failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    let bugs = parsed.as_array().expect("expected JSON array");
    assert_eq!(bugs.len(), 1, "duplicate bug should be deduplicated");
    assert_eq!(bugs[0]["id"], 42);
}
