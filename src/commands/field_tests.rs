#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::FieldAction;
use crate::test_helpers::setup_test_env;
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
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a1.writers(),
    )
    .await;
    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok());
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert!(parsed.as_array().unwrap().len() >= 3);
    assert_eq!(parsed[0]["name"], "NEW");
}

#[tokio::test]
async fn field_aliases_succeeds_without_server() {
    let _lock = crate::ENV_LOCK.lock().await;
    let action = FieldAction::Aliases;
    let mut __io_a2 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a2.writers(),
    )
    .await;
    let output = __io_a2.out_str().to_string();
    assert!(result.is_ok());
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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
    let mut __io_a3 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Table,
        None,
        &mut __io_a3.writers(),
    )
    .await;
    let output = __io_a3.out_str().to_string();
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
    let mut __io_a4 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a4.writers(),
    )
    .await;
    let output = __io_a4.out_str().to_string();
    assert!(result.is_ok());
    assert!(
        !output.contains("No values for field"),
        "JSON format must not emit the table-style 'No values' message; got: {output:?}"
    );
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert!(parsed.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn field_list_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug_status"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = FieldAction::List {
        name: "status".to_string(),
    };
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}
