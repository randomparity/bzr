#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::ComponentAction;
use crate::error::BzrError;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

/// Mock `GET /rest/product?names=MyApp` returning a product with two
/// components.
async fn mount_product_with_components(mock: &wiremock::MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .and(query_param("names", "MyApp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{
                "id": 1,
                "name": "MyApp",
                "description": "App",
                "is_active": true,
                "components": [
                    {"id": 10, "name": "Backend", "description": "be", "is_active": true,
                     "default_assignee": "dev@example.com"},
                    {"id": 11, "name": "Frontend", "description": "fe", "is_active": false,
                     "default_assignee": null}
                ],
                "versions": [],
                "milestones": []
            }]
        })))
        .mount(mock)
        .await;
}

#[tokio::test]
async fn component_list_returns_components() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_product_with_components(&mock).await;

    let action = ComponentAction::List {
        product: "MyApp".into(),
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut __io.writers()).await;
    assert!(result.is_ok(), "list should succeed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(__io.out_str().trim()).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 2);
    assert_eq!(parsed[0]["name"], "Backend");
}

#[tokio::test]
async fn component_view_returns_one_component() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_product_with_components(&mock).await;

    let action = ComponentAction::View {
        product: "MyApp".into(),
        name: "Frontend".into(),
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut __io.writers()).await;
    assert!(result.is_ok(), "view should succeed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(__io.out_str().trim()).unwrap();
    assert_eq!(parsed["id"], 11);
    assert_eq!(parsed["name"], "Frontend");
}

#[tokio::test]
async fn component_view_unknown_name_is_not_found() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_product_with_components(&mock).await;

    let action = ComponentAction::View {
        product: "MyApp".into(),
        name: "Nonexistent".into(),
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut __io.writers()).await;
    assert!(matches!(
        result,
        Err(BzrError::NotFound {
            resource: "component",
            ..
        })
    ));
}

#[tokio::test]
async fn component_create_succeeds() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/component"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 42})))
        .mount(&mock)
        .await;

    let action = ComponentAction::Create {
        product: "TestProduct".to_string(),
        name: "Backend".to_string(),
        description: "Backend component".to_string(),
        default_assignee: "dev@test.com".to_string(),
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
    assert_eq!(parsed["id"], 42);
}

#[tokio::test]
async fn component_create_dry_run_makes_no_write_and_marks_payload() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/component"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 42})))
        .expect(0)
        .mount(&mock)
        .await;

    let action = ComponentAction::Create {
        product: "TestProduct".to_string(),
        name: "Backend".to_string(),
        description: "Backend component".to_string(),
        default_assignee: "dev@test.com".to_string(),
    };
    crate::commands::runtime::dry_run::set(true);
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut io.writers()).await;
    crate::commands::runtime::dry_run::set(false);

    assert!(
        result.is_ok(),
        "dry-run component create failed: {result:?}"
    );
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["resource"], "component");
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([]));
    assert_eq!(parsed["changes"]["name"], "Backend");
}

#[tokio::test]
async fn component_create_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/component"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = ComponentAction::Create {
        product: "TestProduct".to_string(),
        name: "Backend".to_string(),
        description: "Backend component".to_string(),
        default_assignee: "dev@test.com".to_string(),
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

#[tokio::test]
async fn component_update_succeeds() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/component/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 10})))
        .mount(&mock)
        .await;

    let action = ComponentAction::Update {
        id: 10,
        name: Some("Updated".to_string()),
        description: None,
        default_assignee: None,
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
    assert_eq!(parsed["id"], 10);
    assert_eq!(parsed["action"], "updated");
}

#[tokio::test]
async fn component_update_dry_run_makes_no_write_and_marks_payload() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/component/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 10})))
        .expect(0)
        .mount(&mock)
        .await;

    let action = ComponentAction::Update {
        id: 10,
        name: Some("Updated".to_string()),
        description: None,
        default_assignee: Some("owner@test.com".to_string()),
    };
    crate::commands::runtime::dry_run::set(true);
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut io.writers()).await;
    crate::commands::runtime::dry_run::set(false);

    assert!(
        result.is_ok(),
        "dry-run component update failed: {result:?}"
    );
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["resource"], "component");
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([10]));
    assert_eq!(parsed["changes"]["name"], "Updated");
    assert_eq!(parsed["changes"]["default_assignee"], "owner@test.com");
}

#[tokio::test]
async fn component_update_without_fields_is_rejected() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let action = ComponentAction::Update {
        id: 10,
        name: None,
        description: None,
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut io.writers()).await;

    assert!(
        matches!(result, Err(crate::error::BzrError::InputValidation(ref msg))
            if msg.contains("no fields to update")),
        "expected input validation, got {result:?}"
    );
}
