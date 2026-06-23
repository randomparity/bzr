#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::CommentAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn comment_add_with_body() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 100})))
        .mount(&mock)
        .await;

    let action = CommentAction::Add {
        bug_id: 42,
        body: Some("Test comment".to_string()),
        body_file: None,
        private: false,
    };
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn comment_add_empty_body_is_rejected() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    // No mock needed — execute should reject before making any API call.
    let action = CommentAction::Add {
        bug_id: 42,
        body: Some("   ".to_string()),
        body_file: None,
        private: false,
    };
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "empty body should be rejected");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("empty comment"),
        "expected 'empty comment' error, got: {err}"
    );
}

#[tokio::test]
async fn comment_add_api_error_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 100,
            "message": "Bug #42 does not exist."
        })))
        .mount(&mock)
        .await;

    let action = CommentAction::Add {
        bug_id: 42,
        body: Some("Test comment".to_string()),
        body_file: None,
        private: false,
    };
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}
