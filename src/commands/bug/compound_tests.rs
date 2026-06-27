#![expect(clippy::unwrap_used, clippy::panic)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::commands::runtime::invocation::CommandContext;
use crate::error::BzrError;
use crate::types::attachment::UploadAttachmentParams;
use crate::types::comment::AddCommentParams;
use crate::types::output::OutputFormat;

use super::{create_with_sub_steps, CompoundPlan};

fn sample_params() -> crate::types::bug::CreateBugParams {
    crate::types::bug::CreateBugParams {
        product: "P".into(),
        component: "C".into(),
        summary: "S".into(),
        version: "unspecified".into(),
        ..Default::default()
    }
}

fn comment_only_plan() -> CompoundPlan {
    CompoundPlan {
        comment: Some(AddCommentParams {
            text: "first note".into(),
            is_private: false,
        }),
        attachments: vec![],
    }
}

fn attachment(file_name: &str) -> UploadAttachmentParams {
    UploadAttachmentParams {
        bug_id: 0,
        file_name: file_name.into(),
        summary: file_name.into(),
        content_type: "text/plain".into(),
        data: b"data".to_vec(),
        flags: vec![],
        is_private: false,
        comment: None,
        is_patch: false,
    }
}

async fn mock_create(mock: &wiremock::MockServer, id: u64) {
    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({ "id": id })))
        .mount(mock)
        .await;
}

#[tokio::test]
async fn create_then_comment_500_exits_11() {
    let (_lock, mock, _tmp) = crate::test_helpers::setup_test_env().await;
    mock_create(&mock, 42).await;
    Mock::given(method("POST"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock)
        .await;
    let ctx = CommandContext::new(None, OutputFormat::Json, None);
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = create_with_sub_steps(
        &sample_params(),
        comment_only_plan(),
        &ctx,
        &mut io.writers(),
    )
    .await
    .unwrap_err();
    assert_eq!(err.exit_code(), 11);
    match &err {
        BzrError::BatchPartialFailure { succeeded, failed } => {
            assert_eq!(*succeeded, 1);
            assert_eq!(*failed, 1);
        }
        other => panic!("expected BatchPartialFailure, got {other:?}"),
    }
    // The created bug ID must reach stderr (the recovery handle).
    assert!(io.err_str().contains("42"), "stderr was: {}", io.err_str());
    // stdout carries the compound result with the ID and the failed sub-step.
    let data = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(data["id"], 42);
    assert_eq!(data["failed"][0]["step"], "comment");
}

#[tokio::test]
async fn full_success_emits_plain_action_result() {
    let (_lock, mock, _tmp) = crate::test_helpers::setup_test_env().await;
    mock_create(&mock, 7).await;
    Mock::given(method("POST"))
        .and(path("/rest/bug/7/comment"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({ "id": 100 })))
        .mount(&mock)
        .await;
    let ctx = CommandContext::new(None, OutputFormat::Json, None);
    let mut io = crate::test_helpers::CapturedIo::new();
    create_with_sub_steps(
        &sample_params(),
        comment_only_plan(),
        &ctx,
        &mut io.writers(),
    )
    .await
    .unwrap();
    let data = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(data["id"], 7);
    assert_eq!(data["action"], "created");
    // Success uses the plain ActionResult shape — no `failed` key.
    assert!(data.get("failed").is_none(), "data was: {data}");
}

#[tokio::test]
async fn attachment_500_exits_11_naming_the_file() {
    let (_lock, mock, _tmp) = crate::test_helpers::setup_test_env().await;
    mock_create(&mock, 9).await;
    Mock::given(method("POST"))
        .and(path("/rest/bug/9/attachment"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&mock)
        .await;
    let plan = CompoundPlan {
        comment: None,
        attachments: vec![attachment("trace.log")],
    };
    let ctx = CommandContext::new(None, OutputFormat::Json, None);
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = create_with_sub_steps(&sample_params(), plan, &ctx, &mut io.writers())
        .await
        .unwrap_err();
    assert_eq!(err.exit_code(), 11);
    assert!(
        io.err_str().contains("trace.log"),
        "stderr: {}",
        io.err_str()
    );
    let data = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(data["id"], 9);
    assert_eq!(data["failed"][0]["step"], "attachment");
    assert_eq!(data["failed"][0]["file"], "trace.log");
}

#[tokio::test]
async fn dry_run_makes_no_network_calls_and_previews_sub_steps() {
    let (_lock, mock, _tmp) = crate::test_helpers::setup_test_env().await;
    // No mocks mounted: any request would 404 and surface as an error.
    let ctx = CommandContext::new(None, OutputFormat::Json, None).with_dry_run(true);
    let plan = CompoundPlan {
        comment: Some(AddCommentParams {
            text: "preview note".into(),
            is_private: false,
        }),
        attachments: vec![attachment("trace.log")],
    };
    let mut io = crate::test_helpers::CapturedIo::new();
    create_with_sub_steps(&sample_params(), plan, &ctx, &mut io.writers())
        .await
        .unwrap();
    assert_eq!(mock.received_requests().await.unwrap().len(), 0);
    let data = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(data["action"], "dry-run");
    assert_eq!(data["changes"]["comment"], "preview note");
    assert_eq!(data["changes"]["attachments"][0]["file_name"], "trace.log");
    assert_eq!(data["changes"]["attachments"][0]["size"], 4);
}
