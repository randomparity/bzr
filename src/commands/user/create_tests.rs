#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::UserAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn write_json_file(tmp: &tempfile::TempDir, json: &str) -> String {
    let path = tmp.path().join("input.json");
    std::fs::write(&path, json).unwrap();
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn user_create_sends_post() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 99})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = UserAction::Create {
        from_json: None,
        email: Some("new@test.com".into()),
        login: None,
        full_name: Some("New User".into()),
        password: None,
    };
    let mut __io_a2 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::user::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a2.writers(),
    )
    .await;
    let output = __io_a2.out_str().to_string();
    assert!(result.is_ok(), "user create failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["action"], "created");
    assert_eq!(parsed["id"], 99);
}

#[tokio::test]
async fn user_create_dry_run_makes_no_write_and_marks_payload() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 99})))
        .expect(0)
        .mount(&mock)
        .await;

    let action = UserAction::Create {
        from_json: None,
        email: Some("new@test.com".into()),
        login: Some("newuser".into()),
        full_name: Some("New User".into()),
        password: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::user::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "dry-run user create failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["resource"], "user");
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([]));
    assert_eq!(parsed["changes"]["email"], "new@test.com");
    assert_eq!(parsed["changes"]["login"], "newuser");
}

#[tokio::test]
async fn user_create_from_json_sends_merged_body() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/user"))
        .and(body_json(serde_json::json!({
            "email": "cli@test.com",
            "login": "json-login",
            "full_name": "JSON User",
            "password": "secret"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 99})))
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"email":"json@test.com","login":"json-login","full_name":"JSON User","password":"secret"}"#;
    let action = UserAction::Create {
        from_json: Some(write_json_file(&tmp, json)),
        email: Some("cli@test.com".into()),
        login: None,
        full_name: None,
        password: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::user::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "user create from JSON failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed["id"], 99);
}

#[tokio::test]
async fn user_from_json_rejects_unknown_field() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let json = r#"{"email":"new@test.com","bogus":true}"#;
    let action = UserAction::Create {
        from_json: Some(write_json_file(&tmp, json)),
        email: None,
        login: None,
        full_name: None,
        password: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::user::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
async fn user_from_json_rejects_array_shape() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let action = UserAction::Create {
        from_json: Some(write_json_file(&tmp, "[]")),
        email: None,
        login: None,
        full_name: None,
        password: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::user::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(crate::error::BzrError::InputValidation(ref msg))
            if msg.contains("expects a JSON object")),
        "expected object-shape validation, got {result:?}"
    );
}
