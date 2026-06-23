#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::GroupAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

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
    let result = crate::commands::group::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "group add_user failed: {result:?}");
}

#[tokio::test]
async fn group_add_user_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/user/alice%40test%2Ecom"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = GroupAction::AddUser {
        group: "admin".into(),
        user: "alice@test.com".into(),
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
