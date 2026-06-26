#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_json, method, path};
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

#[tokio::test]
async fn component_create_succeeds() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/component"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 42})))
        .mount(&mock)
        .await;

    let action = ComponentAction::Create {
        from_json: None,
        product: Some("TestProduct".to_string()),
        name: Some("Backend".to_string()),
        description: Some("Backend component".to_string()),
        default_assignee: Some("dev@test.com".to_string()),
    };
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a1.writers(),
    )
    .await;
    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(&output);
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
        from_json: None,
        product: Some("TestProduct".to_string()),
        name: Some("Backend".to_string()),
        description: Some("Backend component".to_string()),
        default_assignee: Some("dev@test.com".to_string()),
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "dry-run component create failed: {result:?}"
    );
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["resource"], "component");
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([]));
    assert_eq!(parsed["changes"]["name"], "Backend");
}

#[tokio::test]
async fn component_create_from_json_sends_merged_body() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/component"))
        .and(body_json(serde_json::json!({
            "product": "FromCli",
            "name": "Backend",
            "description": "From JSON",
            "default_assignee": "dev@test.com"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 42})))
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"product":"FromJson","name":"Backend","description":"From JSON","default_assignee":"dev@test.com"}"#;
    let action = ComponentAction::Create {
        from_json: Some(write_json_file(&tmp, json)),
        product: Some("FromCli".to_string()),
        name: None,
        description: None,
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "component create from JSON failed: {result:?}"
    );
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["id"], 42);
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
        from_json: None,
        product: Some("TestProduct".to_string()),
        name: Some("Backend".to_string()),
        description: Some("Backend component".to_string()),
        default_assignee: Some("dev@test.com".to_string()),
    };
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn component_from_json_rejects_unknown_field() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let json = r#"{"product":"P","name":"C","description":"D","default_assignee":"dev@test.com","bogus":true}"#;
    let action = ComponentAction::Create {
        from_json: Some(write_json_file(&tmp, json)),
        product: None,
        name: None,
        description: None,
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
async fn component_from_json_missing_required_field_names_cli_flag() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let json = r#"{"product":"P","name":"C","description":"D"}"#;
    let action = ComponentAction::Create {
        from_json: Some(write_json_file(&tmp, json)),
        product: None,
        name: None,
        description: None,
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
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
        matches!(result, Err(BzrError::InputValidation(ref msg))
            if msg.contains("'default_assignee' is required")
                && msg.contains("--default-assignee")),
        "expected missing field validation, got {result:?}"
    );
}

#[tokio::test]
async fn component_from_json_rejects_array_shape() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let action = ComponentAction::Create {
        from_json: Some(write_json_file(&tmp, "[]")),
        product: None,
        name: None,
        description: None,
        default_assignee: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::component::execute(
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
