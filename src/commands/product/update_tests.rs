#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::ProductAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn write_json_file(tmp: &tempfile::TempDir, json: &str) -> String {
    let path = tmp.path().join("input.json");
    std::fs::write(&path, json).unwrap();
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn product_update_succeeds() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/product/Firefox"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{"id": 1, "changes": {}}]
        })))
        .mount(&mock)
        .await;

    let action = ProductAction::Update {
        from_json: None,
        name: Some("Firefox".to_string()),
        description: Some("Updated".to_string()),
        default_milestone: None,
        is_open: None,
    };
    let mut __io_a4 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::product::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a4.writers(),
    )
    .await;
    let output = __io_a4.out_str().to_string();
    assert!(result.is_ok());
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["action"], "updated");
}

#[tokio::test]
async fn product_update_dry_run_makes_no_write_and_marks_payload() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/product/Firefox"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{"id": 1, "changes": {}}]
        })))
        .expect(0)
        .mount(&mock)
        .await;

    let action = ProductAction::Update {
        from_json: None,
        name: Some("Firefox".to_string()),
        description: Some("Updated".to_string()),
        default_milestone: None,
        is_open: Some(false),
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::product::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(
            Some("missing"),
            OutputFormat::Json,
            None,
        )
        .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "dry-run product update failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["resource"], "product");
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([]));
    assert_eq!(parsed["changes"]["description"], "Updated");
    assert_eq!(parsed["changes"]["is_open"], false);
}

#[tokio::test]
async fn product_update_from_json_uses_json_target() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/product/Firefox"))
        .and(body_json(serde_json::json!({
            "description": "Updated",
            "default_milestone": "2.0",
            "is_open": false
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{"id": 1, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"name":"Firefox","description":"Updated","is_open":false}"#;
    let action = ProductAction::Update {
        from_json: Some(write_json_file(&tmp, json)),
        name: None,
        description: None,
        default_milestone: Some("2.0".to_string()),
        is_open: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::product::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "product update from JSON failed: {result:?}"
    );
}

#[tokio::test]
async fn product_update_from_json_rejects_positional_and_json_target() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let json = r#"{"name":"FromJson","description":"Updated"}"#;
    let action = ProductAction::Update {
        from_json: Some(write_json_file(&tmp, json)),
        name: Some("FromCli".to_string()),
        description: None,
        default_milestone: None,
        is_open: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::product::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(crate::error::BzrError::InputValidation(ref msg))
            if msg.contains("cannot combine positional product name")),
        "expected target conflict, got {result:?}"
    );
}

#[tokio::test]
async fn product_update_without_fields_is_rejected() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let action = ProductAction::Update {
        from_json: None,
        name: Some("Firefox".to_string()),
        description: None,
        default_milestone: None,
        is_open: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::product::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(
            Some("missing"),
            OutputFormat::Json,
            None,
        ),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(crate::error::BzrError::InputValidation(ref msg))
            if msg.contains("no fields to update")),
        "expected input validation, got {result:?}"
    );
}
