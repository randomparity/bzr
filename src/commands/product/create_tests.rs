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
async fn product_create_returns_id() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 5})))
        .mount(&mock)
        .await;

    let action = ProductAction::Create {
        from_json: None,
        name: Some("NewProduct".to_string()),
        description: Some("New product".to_string()),
        version: Some("1.0".to_string()),
        is_open: Some(true),
    };
    let mut __io_a3 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::product::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a3.writers(),
    )
    .await;
    let output = __io_a3.out_str().to_string();
    assert!(result.is_ok());
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["id"], 5);
}

#[tokio::test]
async fn product_create_dry_run_makes_no_write_and_marks_payload() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 5})))
        .expect(0)
        .mount(&mock)
        .await;

    let action = ProductAction::Create {
        from_json: None,
        name: Some("NewProduct".to_string()),
        description: Some("New product".to_string()),
        version: Some("1.0".to_string()),
        is_open: Some(true),
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::product::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "dry-run product create failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["resource"], "product");
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([]));
    assert_eq!(parsed["changes"]["name"], "NewProduct");
}

#[tokio::test]
async fn product_create_from_json_sends_merged_body() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/product"))
        .and(body_json(serde_json::json!({
            "name": "FromCli",
            "description": "From JSON",
            "version": "2.0",
            "is_open": false
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 8})))
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"name":"FromJson","description":"From JSON","version":"2.0","is_open":false}"#;
    let action = ProductAction::Create {
        from_json: Some(write_json_file(&tmp, json)),
        name: Some("FromCli".to_string()),
        description: None,
        version: None,
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
        "product create from JSON failed: {result:?}"
    );
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["id"], 8);
    assert_eq!(parsed["action"], "created");
}

#[tokio::test]
async fn product_from_json_rejects_unknown_field() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let json = r#"{"name":"P","description":"D","bogus":true}"#;
    let action = ProductAction::Create {
        from_json: Some(write_json_file(&tmp, json)),
        name: None,
        description: None,
        version: None,
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
            if msg.contains("bogus") || msg.contains("unknown field")),
        "expected unknown field validation, got {result:?}"
    );
}

#[tokio::test]
async fn product_from_json_rejects_array_shape() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let action = ProductAction::Create {
        from_json: Some(write_json_file(&tmp, "[]")),
        name: None,
        description: None,
        version: None,
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
            if msg.contains("expects a JSON object")),
        "expected object-shape validation, got {result:?}"
    );
}
