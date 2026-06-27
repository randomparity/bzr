use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::AttachmentAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn attachment_view_returns_metadata_without_data() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/9876"))
        .and(query_param("exclude_fields", "data"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": {
                "9876": {
                    "id": 9876,
                    "bug_id": 42,
                    "file_name": "patch.diff",
                    "summary": "Fix patch",
                    "content_type": "text/x-diff",
                    "creator": "dev@test.com",
                    "creation_time": "2025-01-01T00:00:00Z",
                    "is_patch": true,
                    "is_private": false,
                    "size": 1024
                }
            }
        })))
        .mount(&mock)
        .await;

    let action = AttachmentAction::View {
        attachment_id: 9876,
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(result.is_ok(), "view should succeed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(__io.out_str());
    assert_eq!(parsed["id"], 9876);
    assert_eq!(parsed["file_name"], "patch.diff");
    assert_eq!(parsed["size"], 1024);
    assert!(
        parsed.get("data").is_none(),
        "metadata JSON must omit the data field, got: {parsed}"
    );
}

#[tokio::test]
async fn attachment_view_table_renders_metadata() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/9876"))
        .and(query_param("exclude_fields", "data"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": {
                "9876": {
                    "id": 9876,
                    "bug_id": 42,
                    "file_name": "patch.diff",
                    "summary": "Fix the crash",
                    "content_type": "text/x-diff",
                    "creator": "dev@test.com",
                    "creation_time": "2025-01-01T00:00:00Z",
                    "last_change_time": "2025-02-02T00:00:00Z",
                    "is_patch": true,
                    "is_private": true,
                    "size": 2048
                }
            }
        })))
        .mount(&mock)
        .await;

    let action = AttachmentAction::View {
        attachment_id: 9876,
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None),
        &mut __io.writers(),
    )
    .await;
    assert!(result.is_ok(), "view table should succeed: {result:?}");
    let out = __io.out_str().to_string();
    assert!(out.contains("9876"), "table should show id: {out}");
    assert!(
        out.contains("Fix the crash"),
        "table should show summary: {out}"
    );
    assert!(
        out.contains("patch.diff"),
        "table should show file name: {out}"
    );
    assert!(out.contains("[PATCH]"), "table should flag patch: {out}");
    assert!(
        out.contains("[PRIVATE]"),
        "table should flag private: {out}"
    );
}
