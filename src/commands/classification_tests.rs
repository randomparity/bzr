#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::ClassificationAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn classification_view_returns_data() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/classification/Unclassified"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "classifications": [{
                "id": 1,
                "name": "Unclassified",
                "description": "Not yet classified",
                "products": []
            }]
        })))
        .mount(&mock)
        .await;

    let action = ClassificationAction::View {
        name: "Unclassified".to_string(),
    };
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a1.writers(),
    )
    .await;
    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["name"], "Unclassified");
    assert_eq!(parsed["description"], "Not yet classified");
}

#[tokio::test]
async fn classification_view_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/classification/Missing"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = ClassificationAction::View {
        name: "Missing".to_string(),
    };
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn classification_list_returns_sorted_json() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/classification"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{"values": [
                {"name": "Acme", "sort_key": 5},
                {"name": "Unclassified", "sort_key": 0}
            ]}]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/classification/Acme"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "classifications": [{"id": 2, "name": "Acme", "description": "Acme group", "sort_key": 5,
                "products": [{"id": 9, "name": "W", "description": "w"}]}]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/classification/Unclassified"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "classifications": [{"id": 1, "name": "Unclassified", "description": "Default", "sort_key": 0, "products": []}]
        })))
        .mount(&mock)
        .await;

    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &ClassificationAction::List,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(result.is_ok(), "list should succeed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(__io.out_str());
    assert_eq!(parsed.as_array().unwrap().len(), 2);
    assert_eq!(parsed[0]["name"], "Unclassified");
    assert_eq!(parsed[1]["name"], "Acme");
    assert_eq!(parsed[1]["products"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn classification_list_notes_disabled_when_only_unclassified() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/classification"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{"values": [{"name": "Unclassified", "sort_key": 0}]}]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/classification/Unclassified"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "classifications": [{"id": 1, "name": "Unclassified", "description": "Default", "sort_key": 0, "products": []}]
        })))
        .mount(&mock)
        .await;

    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &ClassificationAction::List,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Table, None),
        &mut __io.writers(),
    )
    .await;
    assert!(result.is_ok());
    assert!(
        __io.err_str().contains("classifications disabled"),
        "expected disabled note on stderr, got: {}",
        __io.err_str()
    );
}
