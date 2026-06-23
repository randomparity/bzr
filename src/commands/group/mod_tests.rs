#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_json, method, path, query_param};
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
async fn group_view_returns_info() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .and(query_param("names", "admin"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "groups": [{
                "id": 1,
                "name": "admin",
                "description": "Admin group",
                "is_active": true,
                "membership": []
            }]
        })))
        .mount(&mock)
        .await;

    let action = GroupAction::View {
        group: "admin".to_string(),
    };
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a1.writers(),
    )
    .await;
    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok(), "group_view failed: {result:?}");
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["name"], "admin");
    assert_eq!(parsed["description"], "Admin group");
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
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a2.writers(),
    )
    .await;
    let output = __io_a2.out_str().to_string();
    assert!(result.is_ok(), "group create failed: {result:?}");
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "group create from JSON failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
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
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "dry-run group create failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["resource"], "group");
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([]));
    assert_eq!(parsed["changes"]["name"], "new-group");
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
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = super::execute(
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
    let result = super::execute(
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
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "dry-run group update failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
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
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(crate::error::BzrError::InputValidation(ref msg))
            if msg.contains("no fields to update")),
        "expected input validation, got {result:?}"
    );
}

#[tokio::test]
async fn group_add_user_sends_put() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    // add_user_to_group sends PUT /rest/user/{user} with group membership body
    Mock::given(method("PUT"))
        .and(path("/rest/user/alice%40test%2Ecom"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"users": [{"id": 1, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let action = GroupAction::AddUser {
        group: "admin".into(),
        user: "alice@test.com".into(),
    };
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "group add_user failed: {result:?}");
}

#[tokio::test]
async fn group_remove_user_sends_put() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/user/bob%40test%2Ecom"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"users": [{"id": 2, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let action = GroupAction::RemoveUser {
        group: "admin".into(),
        user: "bob@test.com".into(),
    };
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "group remove_user failed: {result:?}");
}

#[tokio::test]
async fn group_list_users_returns_members() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [
                {"id": 10, "name": "alice@test.com", "real_name": "Alice", "email": "alice@test.com"},
                {"id": 11, "name": "bob@test.com", "real_name": "Bob", "email": "bob@test.com"}
            ]
        })))
        .mount(&mock)
        .await;

    let action = GroupAction::ListUsers {
        group: "admin".to_string(),
        details: false,
    };
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "group list_users failed: {result:?}");
}

#[tokio::test]
async fn group_list_users_with_details() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [
                {"id": 10, "name": "alice@test.com", "real_name": "Alice", "email": "alice@test.com", "can_login": true}
            ]
        })))
        .mount(&mock)
        .await;

    let action = GroupAction::ListUsers {
        group: "admin".to_string(),
        details: true,
    };
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "group list_users --details failed: {result:?}"
    );
}

#[tokio::test]
async fn group_view_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = GroupAction::View {
        group: "admin".to_string(),
    };
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
async fn group_view_malformed_json_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&mock)
        .await;

    let action = GroupAction::View {
        group: "admin".to_string(),
    };
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}
