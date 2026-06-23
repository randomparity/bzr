#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::CommentAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn comment_list_returns_comments() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": {
                    "comments": [{
                        "id": 1,
                        "bug_id": 42,
                        "text": "Hello world",
                        "creator": "user@test.com",
                        "creation_time": "2025-01-01T00:00:00Z",
                        "is_private": false,
                        "count": 0
                    }]
                }
            }
        })))
        .mount(&mock)
        .await;

    let action = CommentAction::List {
        bug_id: 42,
        since: None,
    };
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
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
    assert_eq!(parsed[0]["text"], "Hello world");
    assert_eq!(parsed[0]["creator"], "user@test.com");
}

#[tokio::test]
async fn comment_list_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = CommentAction::List {
        bug_id: 42,
        since: None,
    };
    let result = crate::commands::comment::execute(
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
async fn comment_list_rejects_malformed_since_with_exit_code_7() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = CommentAction::List {
        bug_id: 42,
        since: Some("nope".into()),
    };
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 7);
    let msg = err.to_string();
    assert!(msg.contains("--since"), "error should name the flag: {msg}");
    assert!(
        msg.contains("nope"),
        "error should echo the offending input: {msg}"
    );
}
