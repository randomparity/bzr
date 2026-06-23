#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{AttachmentAction, AttachmentUpdateArgs};
use crate::test_helpers::{setup_empty_config_env, setup_test_env};
use crate::types::OutputFormat;

#[tokio::test]
async fn attachment_update_succeeds() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/bug/attachment/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": [{"id": 99, "changes": {}}]
        })))
        .mount(&mock)
        .await;

    let action = AttachmentAction::Update(AttachmentUpdateArgs {
        id: 99,
        summary: Some("Updated summary".into()),
        file_name: None,
        content_type: None,
        obsolete: false,
        no_obsolete: false,
        patch: false,
        no_patch: false,
        private: false,
        no_private: false,
        flag: vec![],
    });
    let mut __io_a3 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a3.writers(),
    )
    .await;
    let output = __io_a3.out_str().to_string();
    assert!(result.is_ok());
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["id"], 99);
    assert_eq!(parsed["action"], "updated");
}

#[tokio::test]
async fn attachment_update_no_obsolete_sends_false() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    // `--no-obsolete` resolves to is_obsolete: Some(false), which must reach
    // the body as an explicit `false` (not omitted).
    Mock::given(method("PUT"))
        .and(path("/rest/bug/attachment/7"))
        .and(body_string_contains("\"is_obsolete\":false"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": [{"id": 7, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = AttachmentAction::Update(AttachmentUpdateArgs {
        id: 7,
        summary: None,
        file_name: None,
        content_type: None,
        obsolete: false,
        no_obsolete: true,
        patch: false,
        no_patch: false,
        private: false,
        no_private: false,
        flag: vec![],
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok(), "update --no-obsolete failed: {result:?}");
}

#[tokio::test]
async fn attachment_update_unset_bools_are_omitted() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    // With no bool flags the body must carry none of the tri-state keys, so the
    // server leaves those properties unchanged.
    Mock::given(method("PUT"))
        .and(path("/rest/bug/attachment/8"))
        .and(body_string_contains("\"summary\":\"only summary\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": [{"id": 8, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = AttachmentAction::Update(AttachmentUpdateArgs {
        id: 8,
        summary: Some("only summary".into()),
        file_name: None,
        content_type: None,
        obsolete: false,
        no_obsolete: false,
        patch: false,
        no_patch: false,
        private: false,
        no_private: false,
        flag: vec![],
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    // Confirm the request the mock matched carried no tri-state keys.
    let reqs = mock.received_requests().await.unwrap();
    let body = String::from_utf8_lossy(&reqs[0].body);
    assert!(!body.contains("is_obsolete"), "body: {body}");
    assert!(!body.contains("is_patch"), "body: {body}");
    assert!(!body.contains("is_private"), "body: {body}");
}

#[tokio::test]
async fn attachment_update_without_changes_is_rejected_before_put() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/bug/attachment/8"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
        .expect(0)
        .mount(&mock)
        .await;

    let action = AttachmentAction::Update(AttachmentUpdateArgs {
        id: 8,
        summary: None,
        file_name: None,
        content_type: None,
        obsolete: false,
        no_obsolete: false,
        patch: false,
        no_patch: false,
        private: false,
        no_private: false,
        flag: vec![],
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 7);
    let msg = err.to_string();
    assert!(
        msg.contains("no attachment fields to update"),
        "error should name the missing change: {msg}"
    );
    assert!(
        msg.contains("--summary") && msg.contains("--flag"),
        "error should suggest update flags: {msg}"
    );
}

#[tokio::test]
async fn attachment_update_invalid_flag_fails_before_connect() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    let action = AttachmentAction::Update(AttachmentUpdateArgs {
        id: 8,
        summary: Some("new summary".into()),
        file_name: None,
        content_type: None,
        obsolete: false,
        no_obsolete: false,
        patch: false,
        no_patch: false,
        private: false,
        no_private: false,
        flag: vec!["review".into()],
    });

    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err()
    .to_string();

    assert!(
        err.contains("flag") || err.contains("status"),
        "local flag parse error should win over config lookup, got: {err}"
    );
}
