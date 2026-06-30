#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_json, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::ComponentAction;
use crate::error::BzrError;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn write_json_file(tmp: &tempfile::TempDir, json: &str) -> String {
    let path = tmp.path().join("input.json");
    std::fs::write(&path, json).unwrap();
    path.to_string_lossy().into_owned()
}

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

/// Mock `GET /rest/product?names=MyApp` returning duplicate component names.
async fn mount_product_with_duplicate_components(mock: &wiremock::MockServer) {
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
                    {"id": 12, "name": "Backend", "description": "duplicate", "is_active": true,
                     "default_assignee": "other@example.com"}
                ],
                "versions": [],
                "milestones": []
            }]
        })))
        .mount(mock)
        .await;
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
        from_json: None,
        id: Some(10),
        product: None,
        component: None,
        name: Some("Updated".to_string()),
        description: None,
        default_assignee: None,
    };
    let mut __io_a2 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a2.writers(),
    )
    .await;
    let output = __io_a2.out_str().to_string();
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["id"], 10);
    assert_eq!(parsed["action"], "updated");
}

#[tokio::test]
async fn component_update_by_product_and_component_resolves_id() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_product_with_components(&mock).await;

    Mock::given(method("PUT"))
        .and(path("/rest/component/10"))
        .and(body_json(serde_json::json!({"description": "Updated"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 10})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = ComponentAction::Update {
        from_json: None,
        id: None,
        product: Some("MyApp".to_string()),
        component: Some("Backend".to_string()),
        name: None,
        description: Some("Updated".to_string()),
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "component update by product/name failed: {result:?}"
    );
    let parsed = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["id"], 10);
    assert_eq!(parsed["action"], "updated");
}

#[tokio::test]
async fn component_update_from_json_uses_product_component_target() {
    let (_lock, mock, tmp) = setup_test_env().await;
    mount_product_with_components(&mock).await;

    Mock::given(method("PUT"))
        .and(path("/rest/component/10"))
        .and(body_json(serde_json::json!({"description": "Updated"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 10})))
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"product":"MyApp","component":"Backend","description":"Updated"}"#;
    let action = ComponentAction::Update {
        from_json: Some(write_json_file(&tmp, json)),
        id: None,
        product: None,
        component: None,
        name: None,
        description: None,
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "component update from JSON product/name target failed: {result:?}"
    );
}

#[tokio::test]
async fn component_update_rejects_id_and_product_component_target() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let action = ComponentAction::Update {
        from_json: None,
        id: Some(10),
        product: Some("MyApp".to_string()),
        component: Some("Backend".to_string()),
        name: None,
        description: Some("Updated".to_string()),
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(BzrError::InputValidation { message: ref msg, .. })
            if msg.contains("either component ID or --product/--component")),
        "expected mixed-target validation, got {result:?}"
    );
}

#[tokio::test]
async fn component_update_rejects_product_without_component() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let action = ComponentAction::Update {
        from_json: None,
        id: None,
        product: Some("MyApp".to_string()),
        component: None,
        name: None,
        description: Some("Updated".to_string()),
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(BzrError::InputValidation { message: ref msg, .. })
            if msg.contains("--product requires --component")),
        "expected missing component validation, got {result:?}"
    );
}

#[tokio::test]
async fn component_update_named_target_unknown_component_is_not_found() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_product_with_components(&mock).await;

    let action = ComponentAction::Update {
        from_json: None,
        id: None,
        product: Some("MyApp".to_string()),
        component: Some("Missing".to_string()),
        name: None,
        description: Some("Updated".to_string()),
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(BzrError::NotFound { resource: "component", ref id })
            if id == "MyApp/Missing"),
        "expected component not found, got {result:?}"
    );
}

#[tokio::test]
async fn component_update_named_target_duplicate_component_is_ambiguous() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_product_with_duplicate_components(&mock).await;

    let action = ComponentAction::Update {
        from_json: None,
        id: None,
        product: Some("MyApp".to_string()),
        component: Some("Backend".to_string()),
        name: None,
        description: Some("Updated".to_string()),
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(BzrError::InputValidation { message: ref msg, .. })
            if msg.contains("ambiguous") && msg.contains("numeric component ID")),
        "expected duplicate component ambiguity, got {result:?}"
    );
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
        from_json: None,
        id: Some(10),
        product: None,
        component: None,
        name: Some("Updated".to_string()),
        description: None,
        default_assignee: Some("owner@test.com".to_string()),
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "dry-run component update failed: {result:?}"
    );
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["resource"], "component");
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([10]));
    assert_eq!(parsed["changes"]["name"], "Updated");
    assert_eq!(parsed["changes"]["default_assignee"], "owner@test.com");
}

#[tokio::test]
async fn component_update_named_dry_run_makes_no_read_or_write_and_marks_target() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(&mock)
        .await;
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 10})))
        .expect(0)
        .mount(&mock)
        .await;

    let action = ComponentAction::Update {
        from_json: None,
        id: None,
        product: Some("MyApp".to_string()),
        component: Some("Backend".to_string()),
        name: Some("Renamed".to_string()),
        description: Some("Updated".to_string()),
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "named dry-run component update failed: {result:?}"
    );
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["resource"], "component");
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([]));
    assert_eq!(parsed["changes"]["product"], "MyApp");
    assert_eq!(parsed["changes"]["component"], "Backend");
    assert_eq!(parsed["changes"]["name"], "Renamed");
    assert_eq!(parsed["changes"]["description"], "Updated");
}

#[tokio::test]
async fn component_update_from_json_uses_json_target() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/component/10"))
        .and(body_json(serde_json::json!({
            "name": "Backend",
            "description": "Updated",
            "default_assignee": "owner@test.com"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 10})))
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"id":10,"name":"Backend","description":"Updated"}"#;
    let action = ComponentAction::Update {
        from_json: Some(write_json_file(&tmp, json)),
        id: None,
        product: None,
        component: None,
        name: None,
        description: None,
        default_assignee: Some("owner@test.com".to_string()),
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "component update from JSON failed: {result:?}"
    );
}

#[tokio::test]
async fn component_update_from_json_rejects_positional_and_json_target() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let json = r#"{"id":10,"name":"Backend"}"#;
    let action = ComponentAction::Update {
        from_json: Some(write_json_file(&tmp, json)),
        id: Some(11),
        product: None,
        component: None,
        name: None,
        description: None,
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(crate::error::BzrError::InputValidation { message: ref msg, .. })
            if msg.contains("cannot combine positional component ID")),
        "expected target conflict, got {result:?}"
    );
}

#[tokio::test]
async fn component_update_from_json_rejects_id_and_product_component_target() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let json = r#"{"id":10,"product":"MyApp","component":"Backend","description":"Updated"}"#;
    let action = ComponentAction::Update {
        from_json: Some(write_json_file(&tmp, json)),
        id: None,
        product: None,
        component: None,
        name: None,
        description: None,
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(BzrError::InputValidation { message: ref msg, .. })
            if msg.contains("either component ID or --product/--component")),
        "expected JSON mixed-target validation, got {result:?}"
    );
}

#[tokio::test]
async fn component_update_from_json_rejects_partial_product_component_target() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let json = r#"{"product":"MyApp","description":"Updated"}"#;
    let action = ComponentAction::Update {
        from_json: Some(write_json_file(&tmp, json)),
        id: None,
        product: None,
        component: None,
        name: None,
        description: None,
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(BzrError::InputValidation { message: ref msg, .. })
            if msg.contains("'product' and 'component' must be supplied together")),
        "expected partial JSON name-target validation, got {result:?}"
    );
}

#[tokio::test]
async fn component_update_without_fields_is_rejected() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let action = ComponentAction::Update {
        from_json: None,
        id: Some(10),
        product: None,
        component: None,
        name: None,
        description: None,
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(crate::error::BzrError::InputValidation { message: ref msg, .. })
            if msg.contains("no fields to update")),
        "expected input validation, got {result:?}"
    );
}
