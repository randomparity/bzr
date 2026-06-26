use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::AttachmentAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn attachment_list_returns_attachments() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": [{
                    "id": 100,
                    "bug_id": 42,
                    "file_name": "patch.diff",
                    "summary": "Fix patch",
                    "content_type": "text/x-diff",
                    "creator": "dev@test.com",
                    "creation_time": "2025-01-01T00:00:00Z",
                    "last_change_time": "2025-01-01T00:00:00Z",
                    "is_obsolete": false,
                    "is_patch": true,
                    "size": 1024
                }]
            }
        })))
        .mount(&mock)
        .await;

    let action = AttachmentAction::List { bug_id: 42 };
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a1.writers(),
    )
    .await;
    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["id"], 100);
    assert_eq!(parsed[0]["file_name"], "patch.diff");
    assert_eq!(parsed[0]["creator"], "dev@test.com");
}

#[tokio::test]
async fn attachment_list_api_error_propagates() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/999/attachment"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = AttachmentAction::List { bug_id: 999 };
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}
