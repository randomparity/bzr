#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::GroupAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

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
    let result = crate::commands::group::execute(
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
    let result = crate::commands::group::execute(
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
async fn group_list_users_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = GroupAction::ListUsers {
        group: "admin".to_string(),
        details: false,
    };
    let result = crate::commands::group::execute(
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
