#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::CommentAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn comment_search_tags_returns_matches() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/comment/tags/need"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!(["needinfo", "needreview"])),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let action = CommentAction::SearchTags {
        query: "need".into(),
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(result.is_ok(), "search-tags failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(__io.out_str().trim()).unwrap();
    assert_eq!(
        parsed["items"],
        serde_json::json!(["needinfo", "needreview"])
    );
}

#[tokio::test]
async fn comment_search_tags_empty_returns_no_items() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/comment/tags/zzz"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(1)
        .mount(&mock)
        .await;

    let action = CommentAction::SearchTags {
        query: "zzz".into(),
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(result.is_ok(), "search-tags failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(__io.out_str().trim()).unwrap();
    assert_eq!(parsed["items"], serde_json::json!([]));
}

#[tokio::test]
async fn comment_search_tags_api_error_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/comment/tags/boom"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = CommentAction::SearchTags {
        query: "boom".into(),
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
