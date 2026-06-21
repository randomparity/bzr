#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::ProductAction;
use crate::test_helpers::setup_test_env;
use crate::types::{OutputFormat, ProductListType};

fn write_json_file(tmp: &tempfile::TempDir, json: &str) -> String {
    let path = tmp.path().join("input.json");
    std::fs::write(&path, json).unwrap();
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn product_list_returns_products() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [1, 2]})))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{
                "id": 1,
                "name": "TestProduct",
                "description": "A test product"
            }]
        })))
        .mount(&mock)
        .await;

    let action = ProductAction::List {
        r#type: ProductListType::Accessible,
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
    assert_eq!(parsed[0]["name"], "TestProduct");
}

#[tokio::test]
async fn product_list_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = ProductAction::List {
        r#type: ProductListType::Accessible,
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
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("500") || err.contains("Internal Server Error"),
        "expected HTTP 500 error, got: {err}"
    );
}

#[tokio::test]
async fn product_view_returns_detail() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{
                "id": 1,
                "name": "Firefox",
                "description": "Web browser",
                "is_active": true,
                "components": [],
                "versions": [],
                "milestones": []
            }]
        })))
        .mount(&mock)
        .await;

    let action = ProductAction::View {
        name: "Firefox".to_string(),
    };
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
    assert_eq!(parsed["name"], "Firefox");
    assert_eq!(parsed["description"], "Web browser");
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
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
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
    crate::commands::runtime::dry_run::set(true);
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut io.writers()).await;
    crate::commands::runtime::dry_run::set(false);

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
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut io.writers()).await;

    assert!(
        result.is_ok(),
        "product create from JSON failed: {result:?}"
    );
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["id"], 8);
    assert_eq!(parsed["action"], "created");
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
    crate::commands::runtime::dry_run::set(true);
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        Some("missing"),
        OutputFormat::Json,
        None,
        &mut io.writers(),
    )
    .await;
    crate::commands::runtime::dry_run::set(false);

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
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut io.writers()).await;

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
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut io.writers()).await;

    assert!(
        matches!(result, Err(crate::error::BzrError::InputValidation(ref msg))
            if msg.contains("cannot combine positional product name")),
        "expected target conflict, got {result:?}"
    );
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
    let result = super::execute(
        &action,
        Some("missing"),
        OutputFormat::Json,
        None,
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
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut io.writers()).await;

    assert!(
        matches!(result, Err(crate::error::BzrError::InputValidation(ref msg))
            if msg.contains("expects a JSON object")),
        "expected object-shape validation, got {result:?}"
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
    let result = super::execute(
        &action,
        Some("missing"),
        OutputFormat::Json,
        None,
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(crate::error::BzrError::InputValidation(ref msg))
            if msg.contains("no fields to update")),
        "expected input validation, got {result:?}"
    );
}
