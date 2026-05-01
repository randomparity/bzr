#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::FieldAction;
use crate::test_helpers::{capture_stdout, extract_json, setup_test_env};
use crate::types::OutputFormat;

#[tokio::test]
async fn field_list_returns_values() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug_status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{
                "name": "bug_status",
                "values": [
                    {"name": "NEW"},
                    {"name": "ASSIGNED"},
                    {"name": "RESOLVED"}
                ]
            }]
        })))
        .mount(&mock)
        .await;

    let action = FieldAction::List {
        name: "status".to_string(),
    };
    let (result, output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok());
    let parsed = extract_json(&output);
    assert!(parsed.as_array().unwrap().len() >= 3);
    assert_eq!(parsed[0]["name"], "NEW");
}

#[tokio::test]
async fn field_aliases_succeeds_without_server() {
    let _lock = crate::ENV_LOCK.lock().await;
    let action = FieldAction::Aliases;
    let (result, output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok());
    let parsed = extract_json(&output);
    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty());
    assert_eq!(arr[0]["alias"], "file_loc");
    assert_eq!(arr[0]["api_name"], "bug_file_loc");
}

#[tokio::test]
async fn field_list_table_format_with_empty_values_prints_no_values_message() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug_status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{"name": "bug_status", "values": []}]
        })))
        .mount(&mock)
        .await;
    let action = FieldAction::List {
        name: "status".to_string(),
    };
    let (result, output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Table, None)).await;
    assert!(result.is_ok());
    assert!(
        output.contains("No values for field"),
        "expected 'No values' message, got: {output:?}"
    );
}

#[tokio::test]
async fn field_list_json_format_with_empty_values_emits_empty_array() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug_status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{"name": "bug_status", "values": []}]
        })))
        .mount(&mock)
        .await;
    let action = FieldAction::List {
        name: "status".to_string(),
    };
    let (result, output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok());
    assert!(
        !output.contains("No values for field"),
        "JSON format must not emit the table-style 'No values' message; got: {output:?}"
    );
    let parsed = extract_json(&output);
    assert!(parsed.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn field_list_http_500_returns_error() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug_status"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = FieldAction::List {
        name: "status".to_string(),
    };
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_err());
}
