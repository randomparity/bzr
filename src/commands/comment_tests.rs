#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::CommentAction;
use crate::test_helpers::{capture_stdout, setup_test_env};
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
    let (result, output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed[0]["id"], 1);
    assert_eq!(parsed[0]["text"], "Hello world");
    assert_eq!(parsed[0]["creator"], "user@test.com");
}

#[tokio::test]
async fn comment_add_with_body() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 100})))
        .mount(&mock)
        .await;

    let action = CommentAction::Add {
        bug_id: 42,
        body: Some("Test comment".to_string()),
        private: false,
    };
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn comment_add_empty_body_is_rejected() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    // No mock needed — execute() should reject before making any API call
    let action = CommentAction::Add {
        bug_id: 42,
        body: Some("   ".to_string()),
        private: false,
    };
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_err(), "empty body should be rejected");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("empty comment"),
        "expected 'empty comment' error, got: {err}"
    );
}

#[test]
fn filter_comment_body_strips_html_comments() {
    let raw = "Hello\n<!-- Enter your comment above this line -->\nWorld";
    assert_eq!(super::filter_comment_body(raw), "Hello\nWorld");
}

#[test]
fn filter_comment_body_preserves_normal_text() {
    let raw = "Just a comment\nwith multiple lines";
    assert_eq!(super::filter_comment_body(raw), raw);
}

#[test]
fn filter_comment_body_empty_input() {
    assert_eq!(super::filter_comment_body(""), "");
}

#[tokio::test]
async fn comment_list_http_500_returns_error() {
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
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("500") || err.contains("Internal Server Error"),
        "expected HTTP 500 error, got: {err}"
    );
}

#[tokio::test]
async fn comment_add_api_error_returns_error() {
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
        private: false,
    };
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_err());
}
