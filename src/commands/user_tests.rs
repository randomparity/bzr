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

#[test]
fn resolve_login_denied_text_disable_with_custom_text() {
    assert_eq!(
        super::resolve_login_denied_text(Some(true), Some("Go away")),
        Some("Go away".into())
    );
}

#[test]
fn resolve_login_denied_text_disable_without_custom_text_uses_default() {
    assert_eq!(
        super::resolve_login_denied_text(Some(true), None),
        Some("Account disabled".into())
    );
}

#[test]
fn resolve_login_denied_text_enable_clears_to_empty_string() {
    assert_eq!(
        super::resolve_login_denied_text(Some(false), None),
        Some(String::new())
    );
    assert_eq!(
        super::resolve_login_denied_text(Some(false), Some("ignored")),
        Some(String::new())
    );
}

#[test]
fn resolve_login_denied_text_unset_returns_none() {
    assert_eq!(super::resolve_login_denied_text(None, None), None);
    assert_eq!(
        super::resolve_login_denied_text(None, Some("ignored")),
        None
    );
}

#[tokio::test]
async fn user_search_returns_results() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{
                "id": 1,
                "name": "alice@test.com",
                "real_name": "Alice"
            }]
        })))
        .mount(&mock)
        .await;

    let action = UserAction::Search {
        query: "alice".to_string(),
        details: false,
    };
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a1.writers(),
    )
    .await;
    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed[0]["id"], 1);
    assert_eq!(parsed[0]["name"], "alice@test.com");
}

#[tokio::test]
async fn update_user_disable_login_sends_denied_text() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/user/alice%40test%2Ecom"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = UserAction::Update {
        from_json: None,
        user: Some("alice@test.com".to_string()),
        real_name: None,
        email: None,
        disable_login: Some(true),
        login_denied_text: Some("Go away".to_string()),
    };
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "update with disable_login failed: {result:?}"
    );
}

#[tokio::test]
async fn update_user_enable_login_sends_empty_denied_text() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/user/bob%40test%2Ecom"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = UserAction::Update {
        from_json: None,
        user: Some("bob@test.com".to_string()),
        real_name: None,
        email: None,
        disable_login: Some(false),
        login_denied_text: None,
    };
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "update with enable_login failed: {result:?}"
    );
}

#[tokio::test]
async fn user_update_dry_run_makes_no_write_and_marks_payload() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/user/alice%40test%2Ecom"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(&mock)
        .await;

    let action = UserAction::Update {
        from_json: None,
        user: Some("alice@test.com".to_string()),
        real_name: Some("Alice Smith".to_string()),
        email: None,
        disable_login: Some(true),
        login_denied_text: Some("Closed".to_string()),
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "dry-run user update failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["resource"], "user");
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([]));
    assert_eq!(
        parsed["changes"]["names"],
        serde_json::json!(["alice@test.com"])
    );
    assert_eq!(parsed["changes"]["real_name"], "Alice Smith");
    assert_eq!(parsed["changes"]["login_denied_text"], "Closed");
}

#[tokio::test]
async fn user_update_from_json_uses_json_target() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/user/alice%40test%2Ecom"))
        .and(body_json(serde_json::json!({
            "names": ["alice@test.com"],
            "real_name": "Alice Smith",
            "email": "alice.new@test.com",
            "login_denied_text": "Closed"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"user":"alice@test.com","real_name":"Alice Smith","disable_login":true,"login_denied_text":"Closed"}"#;
    let action = UserAction::Update {
        from_json: Some(write_json_file(&tmp, json)),
        user: None,
        real_name: None,
        email: Some("alice.new@test.com".to_string()),
        disable_login: None,
        login_denied_text: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "user update from JSON failed: {result:?}");
}

#[tokio::test]
async fn user_update_from_json_cli_disable_login_overrides_json_denied_text() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/user/alice%40test%2Ecom"))
        .and(body_json(serde_json::json!({
            "names": ["alice@test.com"],
            "login_denied_text": ""
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"user":"alice@test.com","disable_login":true,"login_denied_text":"Closed"}"#;
    let action = UserAction::Update {
        from_json: Some(write_json_file(&tmp, json)),
        user: None,
        real_name: None,
        email: None,
        disable_login: Some(false),
        login_denied_text: None,
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "CLI disable-login override failed: {result:?}"
    );
}

#[tokio::test]
async fn user_update_from_json_rejects_positional_and_json_target() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let json = r#"{"user":"alice@test.com","real_name":"Alice"}"#;
    let action = UserAction::Update {
        from_json: Some(write_json_file(&tmp, json)),
        user: Some("bob@test.com".to_string()),
        real_name: None,
        email: None,
        disable_login: None,
        login_denied_text: None,
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
            if msg.contains("cannot combine positional user")),
        "expected target conflict, got {result:?}"
    );
}

#[tokio::test]
async fn user_update_without_fields_is_rejected() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let action = UserAction::Update {
        from_json: None,
        user: Some("alice@test.com".to_string()),
        real_name: None,
        email: None,
        disable_login: None,
        login_denied_text: None,
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
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a2.writers(),
    )
    .await;
    let output = __io_a2.out_str().to_string();
    assert!(result.is_ok(), "user create failed: {result:?}");
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "dry-run user create failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
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
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "user create from JSON failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
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
    let result = super::execute(
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
async fn user_search_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = UserAction::Search {
        query: "alice".to_string(),
        details: false,
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
async fn user_search_malformed_json_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
        .mount(&mock)
        .await;

    let action = UserAction::Search {
        query: "alice".to_string(),
        details: false,
    };
    let result = super::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}
