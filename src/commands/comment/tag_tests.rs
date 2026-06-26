#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::CommentAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn comment_tag_add_updates_tags() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/bug/comment/100/tags"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!(["needinfo", "reviewed"])),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let action = CommentAction::Tag {
        comment_id: 100,
        add: vec!["needinfo".into()],
        remove: vec![],
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(result.is_ok(), "comment tag failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(__io.out_str());
    assert_eq!(parsed["comment_id"], 100);
    assert_eq!(parsed["action"], "updated");
    assert_eq!(parsed["tags"], serde_json::json!(["needinfo", "reviewed"]));
}

#[tokio::test]
async fn comment_tag_without_changes_is_rejected_before_put() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/bug/comment/100/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0)
        .mount(&mock)
        .await;

    let action = CommentAction::Tag {
        comment_id: 100,
        add: vec![],
        remove: vec![],
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
    assert!(
        msg.contains("no comment tag changes"),
        "error should name the missing change: {msg}"
    );
    assert!(
        msg.contains("--add") && msg.contains("--remove"),
        "error should suggest tag flags: {msg}"
    );
}
