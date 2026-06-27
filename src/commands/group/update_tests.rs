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
async fn group_update_sends_put() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/group/admin"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"groups": [{"changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let action = GroupAction::Update {
        from_json: None,
        group: Some("admin".into()),
        description: Some("Updated description".into()),
        is_active: None,
    };
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "group update failed: {result:?}");
}

#[tokio::test]
async fn group_update_from_json_uses_json_target() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/group/admin"))
        .and(body_json(serde_json::json!({
            "description": "Updated",
            "is_active": false
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"groups": [{"changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"group":"admin","description":"Updated"}"#;
    let action = GroupAction::Update {
        from_json: Some(write_json_file(&tmp, json)),
        group: None,
        description: None,
        is_active: Some(false),
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "group update from JSON failed: {result:?}");
}

#[tokio::test]
async fn group_update_from_json_rejects_positional_and_json_target() {
    let (_lock, _mock, tmp) = setup_test_env().await;

    let json = r#"{"group":"admin","description":"Updated"}"#;
    let action = GroupAction::Update {
        from_json: Some(write_json_file(&tmp, json)),
        group: Some("other".into()),
        description: None,
        is_active: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(crate::error::BzrError::InputValidation(ref msg))
            if msg.contains("cannot combine positional group")),
        "expected target conflict validation, got {result:?}"
    );
}

#[tokio::test]
async fn group_update_dry_run_makes_no_write_and_marks_payload() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/group/admin"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"groups": [{"changes": {}}]})),
        )
        .expect(0)
        .mount(&mock)
        .await;

    let action = GroupAction::Update {
        from_json: None,
        group: Some("admin".into()),
        description: Some("Updated description".into()),
        is_active: Some(false),
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "dry-run group update failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["resource"], "group");
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([]));
    assert_eq!(parsed["changes"]["description"], "Updated description");
    assert_eq!(parsed["changes"]["is_active"], false);
}

#[tokio::test]
async fn group_update_without_fields_is_rejected() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let action = GroupAction::Update {
        from_json: None,
        group: Some("admin".into()),
        description: None,
        is_active: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(crate::error::BzrError::InputValidation(ref msg))
            if msg.contains("no fields to update")),
        "expected input validation, got {result:?}"
    );
}
