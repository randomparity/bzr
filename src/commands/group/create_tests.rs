#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::GroupAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn write_json_file(tmp: &tempfile::TempDir, json: &str) -> String {
    let path = tmp.path().join("input.json");
    std::fs::write(&path, json).unwrap();
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn group_create_sends_post() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 5})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = GroupAction::Create {
        from_json: None,
        name: Some("new-group".into()),
        description: Some("A test group".into()),
        is_active: Some(true),
    };
    let mut __io_a2 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a2.writers(),
    )
    .await;
    let output = __io_a2.out_str().to_string();
    assert!(result.is_ok(), "group create failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["action"], "created");
    assert_eq!(parsed["id"], 5);
}

#[tokio::test]
async fn group_create_from_json_sends_merged_body() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/group"))
        .and(body_json(serde_json::json!({
            "name": "FromCli",
            "description": "From JSON",
            "is_active": false
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 9})))
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"name":"FromJson","description":"From JSON","is_active":false}"#;
    let action = GroupAction::Create {
        from_json: Some(write_json_file(&tmp, json)),
        name: Some("FromCli".into()),
        description: None,
        is_active: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "group create from JSON failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["id"], 9);
    assert_eq!(parsed["action"], "created");
}

#[tokio::test]
async fn group_create_dry_run_makes_no_write_and_marks_payload() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 5})))
        .expect(0)
        .mount(&mock)
        .await;

    let action = GroupAction::Create {
        from_json: None,
        name: Some("new-group".into()),
        description: Some("A test group".into()),
        is_active: Some(true),
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "dry-run group create failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["resource"], "group");
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([]));
    assert_eq!(parsed["changes"]["name"], "new-group");
}

#[tokio::test]
async fn group_from_json_rejects_unknown_field() {
    let (_lock, _mock, tmp) = setup_test_env().await;

    let json = r#"{"name":"new-group","description":"Group","bogus":true}"#;
    let action = GroupAction::Create {
        from_json: Some(write_json_file(&tmp, json)),
        name: None,
        description: None,
        is_active: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(crate::error::BzrError::InputValidation(ref msg))
            if msg.contains("unknown field") && msg.contains("bogus")),
        "expected unknown field validation, got {result:?}"
    );
}

#[tokio::test]
async fn group_from_json_rejects_array_shape() {
    let (_lock, _mock, tmp) = setup_test_env().await;

    let action = GroupAction::Create {
        from_json: Some(write_json_file(&tmp, r#"[{"name":"new-group"}]"#)),
        name: None,
        description: None,
        is_active: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
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
