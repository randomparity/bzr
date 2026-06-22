#![expect(clippy::unwrap_used)]

use super::*;
use wiremock::matchers::{body_string_contains, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::attachment::{UpdateArgs, UploadArgs};
use crate::cli::AttachmentAction;
use crate::test_helpers::{make_attachment, setup_test_env};
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
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a1.writers(),
    )
    .await;
    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok());
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn attachment_upload_api_error_propagates() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 600,
            "message": "You cannot attach files to this bug."
        })))
        .mount(&mock)
        .await;

    let upload_file = tmp.path().join("upload.txt");
    std::fs::write(&upload_file, "test content").unwrap();

    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: Some("Test".into()),
        content_type: None,
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: None,
        comment_file: None,
        comment_private: false,
        flag: vec![],
    });
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("cannot attach"),
        "expected API error message, got: {err}"
    );
}

#[tokio::test]
async fn attachment_download_api_error_propagates() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/404"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": true,
            "code": 100,
            "message": "Attachment 404 does not exist."
        })))
        .mount(&mock)
        .await;

    let action = AttachmentAction::Download {
        ids: vec![404],
        bug_ids: vec![],
        out: None,
        out_dir: "./attachments".into(),
    };
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn attachment_upload_returns_id() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [200]})))
        .mount(&mock)
        .await;

    let upload_file = tmp.path().join("upload.txt");
    std::fs::write(&upload_file, "test content").unwrap();

    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: Some("Test upload".into()),
        content_type: Some("text/plain".into()),
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: None,
        comment_file: None,
        comment_private: false,
        flag: vec![],
    });
    let mut __io_a2 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a2.writers(),
    )
    .await;
    let output = __io_a2.out_str().to_string();
    assert!(result.is_ok());
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["id"], 200);
}

#[tokio::test]
async fn attachment_upload_with_comment_includes_comment_in_request() {
    use wiremock::matchers::body_string_contains;
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .and(body_string_contains("\"comment\":\"see this\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [201]})))
        .expect(1)
        .mount(&mock)
        .await;

    let upload_file = tmp.path().join("upload.txt");
    std::fs::write(&upload_file, "test content").unwrap();

    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: Some("Test".into()),
        content_type: Some("text/plain".into()),
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: Some("see this".into()),
        comment_file: None,
        comment_private: false,
        flag: vec![],
    });
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "upload with --comment should succeed: {result:?}"
    );
}

#[tokio::test]
async fn attachment_upload_with_comment_file_includes_comment_in_request() {
    use wiremock::matchers::body_string_contains;
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .and(body_string_contains("\"comment\":\"from file body\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [202]})))
        .expect(1)
        .mount(&mock)
        .await;

    let upload_file = tmp.path().join("upload.txt");
    let comment_file = tmp.path().join("comment.txt");
    std::fs::write(&upload_file, "test content").unwrap();
    std::fs::write(&comment_file, "from file body").unwrap();

    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: Some("Test".into()),
        content_type: Some("text/plain".into()),
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: None,
        comment_file: Some(comment_file),
        comment_private: false,
        flag: vec![],
    });
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "upload with --comment-file should succeed: {result:?}"
    );
}

#[tokio::test]
async fn attachment_upload_rejects_whitespace_comment() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("upload.txt");
    std::fs::write(&upload_file, "test content").unwrap();

    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: Some("Test".into()),
        content_type: Some("text/plain".into()),
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: Some(" \n\t".into()),
        comment_file: None,
        comment_private: false,
        flag: vec![],
    });
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(matches!(
        result,
        Err(crate::error::BzrError::InputValidation(_))
    ));
}

#[tokio::test]
async fn attachment_upload_rejects_whitespace_comment_file() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("upload.txt");
    let comment_file = tmp.path().join("comment.txt");
    std::fs::write(&upload_file, "test content").unwrap();
    std::fs::write(&comment_file, " \n\t").unwrap();

    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: Some("Test".into()),
        content_type: Some("text/plain".into()),
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: None,
        comment_file: Some(comment_file),
        comment_private: false,
        flag: vec![],
    });
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(matches!(
        result,
        Err(crate::error::BzrError::InputValidation(_))
    ));
}

#[tokio::test]
async fn attachment_upload_with_is_patch_defaults_content_type_to_text_plain() {
    use wiremock::matchers::body_string_contains;
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .and(body_string_contains("\"is_patch\":true"))
        .and(body_string_contains("\"content_type\":\"text/plain\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [501]})))
        .expect(1)
        .mount(&mock)
        .await;

    let upload_file = tmp.path().join("fix.patch");
    std::fs::write(&upload_file, "diff --git a b").unwrap();

    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: None,
        content_type: None,
        private: false,
        no_private: false,
        patch: true,
        no_patch: false,
        comment: None,
        comment_file: None,
        comment_private: false,
        flag: vec![],
    });
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "upload --is-patch should succeed: {result:?}"
    );
}

#[tokio::test]
async fn attachment_upload_is_patch_with_explicit_content_type_keeps_content_type() {
    use wiremock::matchers::body_string_contains;
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .and(body_string_contains("\"is_patch\":true"))
        .and(body_string_contains(
            "\"content_type\":\"application/octet-stream\"",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [502]})))
        .expect(1)
        .mount(&mock)
        .await;

    let upload_file = tmp.path().join("fix.patch");
    std::fs::write(&upload_file, "binary").unwrap();

    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: None,
        content_type: Some("application/octet-stream".to_string()),
        private: false,
        no_private: false,
        patch: true,
        no_patch: false,
        comment: None,
        comment_file: None,
        comment_private: false,
        flag: vec![],
    });
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "upload --is-patch with explicit ct should succeed: {result:?}"
    );
}

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

    let action = AttachmentAction::Update(UpdateArgs {
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
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
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

    let action = AttachmentAction::Update(UpdateArgs {
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
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut io.writers()).await;
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

    let action = AttachmentAction::Update(UpdateArgs {
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
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut io.writers()).await;
    assert!(result.is_ok());
    // Confirm the request the mock matched carried no tri-state keys.
    let reqs = mock.received_requests().await.unwrap();
    let body = String::from_utf8_lossy(&reqs[0].body);
    assert!(!body.contains("is_obsolete"), "body: {body}");
    assert!(!body.contains("is_patch"), "body: {body}");
    assert!(!body.contains("is_private"), "body: {body}");
}

#[test]
fn guess_content_type_text_plain() {
    assert_eq!(guess_content_type("file.txt"), "text/plain");
    assert_eq!(guess_content_type("script.py"), "text/plain");
    assert_eq!(guess_content_type("main.rs"), "text/plain");
    assert_eq!(guess_content_type("code.c"), "text/plain");
    assert_eq!(guess_content_type("app.js"), "text/plain");
}

#[test]
fn guess_content_type_html() {
    assert_eq!(guess_content_type("page.html"), "text/html");
    assert_eq!(guess_content_type("page.htm"), "text/html");
}

#[test]
fn guess_content_type_json() {
    assert_eq!(guess_content_type("data.json"), "application/json");
}

#[test]
fn guess_content_type_structured_documents() {
    assert_eq!(guess_content_type("config.xml"), "application/xml");
    assert_eq!(guess_content_type("report.pdf"), "application/pdf");
}

#[test]
fn guess_content_type_images() {
    assert_eq!(guess_content_type("photo.png"), "image/png");
    assert_eq!(guess_content_type("photo.jpg"), "image/jpeg");
    assert_eq!(guess_content_type("photo.jpeg"), "image/jpeg");
    assert_eq!(guess_content_type("anim.gif"), "image/gif");
    assert_eq!(guess_content_type("logo.svg"), "image/svg+xml");
}

#[test]
fn guess_content_type_archives() {
    assert_eq!(guess_content_type("file.gz"), "application/gzip");
    assert_eq!(guess_content_type("file.tgz"), "application/gzip");
    assert_eq!(guess_content_type("file.zip"), "application/zip");
    assert_eq!(guess_content_type("file.tar"), "application/x-tar");
}

#[test]
fn guess_content_type_diff() {
    assert_eq!(guess_content_type("fix.patch"), "text/x-diff");
    assert_eq!(guess_content_type("changes.diff"), "text/x-diff");
}

#[test]
fn guess_content_type_unknown() {
    assert_eq!(guess_content_type("file.xyz"), "application/octet-stream");
    assert_eq!(guess_content_type("noext"), "application/octet-stream");
}

#[test]
fn guess_content_type_case_insensitive() {
    assert_eq!(guess_content_type("FILE.TXT"), "text/plain");
    assert_eq!(guess_content_type("image.PNG"), "image/png");
    assert_eq!(guess_content_type("data.JSON"), "application/json");
}

#[tokio::test]
async fn attachment_upload_with_comment_private_flips_privacy() {
    use wiremock::matchers::body_string_contains;
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("p.diff");
    std::fs::write(&upload_file, "diff --git a/x b/x").unwrap();

    // Step 1: POST /rest/bug/42/attachment → returns attachment id 200
    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .and(body_string_contains("\"comment\":\"sensitive\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [200]})))
        .expect(1)
        .mount(&mock)
        .await;

    // Step 2: GET /rest/bug/42/comment → returns comments with one matching attachment_id=200
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": {
                    "comments": [
                        {"id": 554, "bug_id": 42, "text": "old", "is_private": false},
                        {"id": 555, "bug_id": 42, "text": "sensitive", "is_private": false, "attachment_id": 200}
                    ]
                }
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    // Step 3: PUT /rest/bug/42 with comment_is_private: {"555": true}
    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .and(body_string_contains("\"comment_is_private\""))
        .and(body_string_contains("\"555\":true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: Some("test".into()),
        content_type: Some("text/x-diff".into()),
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: Some("sensitive".into()),
        comment_file: None,
        comment_private: true,
        flag: vec![],
    });
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "two-call workflow should succeed: {result:?}"
    );
}

#[tokio::test]
async fn attachment_upload_comment_private_with_comment_file_flips_privacy() {
    use wiremock::matchers::body_string_contains;
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("p.diff");
    let comment_file = tmp.path().join("comment.txt");
    std::fs::write(&upload_file, "diff --git a/x b/x").unwrap();
    std::fs::write(&comment_file, "file sensitive").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .and(body_string_contains("\"comment\":\"file sensitive\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [200]})))
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": {
                    "comments": [
                        {"id": 555, "bug_id": 42, "text": "file sensitive", "is_private": false, "attachment_id": 200}
                    ]
                }
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .and(body_string_contains("\"comment_is_private\""))
        .and(body_string_contains("\"555\":true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: Some("test".into()),
        content_type: Some("text/x-diff".into()),
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: None,
        comment_file: Some(comment_file),
        comment_private: true,
        flag: vec![],
    });
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "comment-file two-call workflow should succeed: {result:?}"
    );
}

#[tokio::test]
async fn attachment_upload_comment_private_without_comment_is_input_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, tmp) = setup_test_env().await;
    let upload_file = tmp.path().join("p.diff");
    std::fs::write(&upload_file, "x").unwrap();

    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: None,
        content_type: None,
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: None,
        comment_file: None,
        comment_private: true,
        flag: vec![],
    });
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(matches!(
        result,
        Err(crate::error::BzrError::InputValidation(_))
    ));
}

#[tokio::test]
async fn attachment_upload_comment_private_partial_failure_propagates_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("p.diff");
    std::fs::write(&upload_file, "x").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [200]})))
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": {
                    "comments": [
                        {"id": 555, "bug_id": 42, "text": "x", "is_private": false, "attachment_id": 200}
                    ]
                }
            }
        })))
        .mount(&mock)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden: editbugs required"))
        .mount(&mock)
        .await;

    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: None,
        content_type: None,
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: Some("x".into()),
        comment_file: None,
        comment_private: true,
        flag: vec![],
    });
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "step-2 failure should propagate");
}

#[tokio::test]
async fn attachment_upload_comment_private_no_matching_comment_is_data_integrity_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("p.diff");
    std::fs::write(&upload_file, "x").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [200]})))
        .mount(&mock)
        .await;

    // Comments fetched successfully, but none has attachment_id matching
    // the just-uploaded attachment. This is a Bugzilla server invariant
    // violation — Bug.add_attachment with a `comment` body must create a
    // comment whose `attachment_id` equals the new attachment's id.
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": {
                    "comments": [
                        {"id": 999, "bug_id": 42, "text": "unrelated", "is_private": false}
                    ]
                }
            }
        })))
        .mount(&mock)
        .await;

    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: None,
        content_type: None,
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: Some("x".into()),
        comment_file: None,
        comment_private: true,
        flag: vec![],
    });
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(matches!(
        result,
        Err(crate::error::BzrError::DataIntegrity(_))
    ));
}

#[tokio::test]
async fn attachment_download_validation_rejects_no_ids_no_bugs() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![],
        out: None,
        out_dir: "./attachments".into(),
    };
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "expected InputValidation");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("specify at least one attachment ID"),
        "unexpected error: {msg}",
    );
}

#[tokio::test]
async fn attachment_download_validation_rejects_out_with_multiple_ids() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let action = AttachmentAction::Download {
        ids: vec![100, 200],
        bug_ids: vec![],
        out: Some("file.bin".into()),
        out_dir: "./attachments".into(),
    };
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "expected InputValidation");
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("--out requires exactly one attachment ID"),
        "unexpected error: {msg}",
    );
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[tokio::test]
async fn write_one_attachment_writes_inline_data_with_att_id_prefix() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let client = super::super::runtime::shared::connect_and_configure(None, None)
        .await
        .unwrap();

    let mut att = make_attachment(
        9876,
        12345,
        "patch.diff",
        "Fix patch",
        Some(b64(b"Hello world")),
    );
    att.content_type = "text/x-diff".into();
    att.size = 11;
    att.is_patch = true;
    let out_dir = tmp.path().to_string_lossy().into_owned();

    let file = super::write_one_attachment(&client, &att, &out_dir)
        .await
        .unwrap();

    let expected_path = tmp.path().join("12345").join("9876.patch.diff");
    assert!(expected_path.exists(), "{expected_path:?} not found");
    assert_eq!(std::fs::read(&expected_path).unwrap(), b"Hello world");
    assert_eq!(file.attachment_id, 9876);
    assert_eq!(file.bytes, 11);
}

#[tokio::test]
async fn write_one_attachment_falls_back_when_data_missing() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/9876"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": {
                "9876": {
                    "id": 9876,
                    "bug_id": 12345,
                    "file_name": "patch.diff",
                    "summary": "Fix patch",
                    "content_type": "text/plain",
                    "size": 11,
                    "data": b64(b"Hello world")
                }
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let client = super::super::runtime::shared::connect_and_configure(None, None)
        .await
        .unwrap();

    let mut att = make_attachment(9876, 12345, "patch.diff", "Fix patch", None);
    att.size = 11;
    let out_dir = tmp.path().to_string_lossy().into_owned();

    let file = super::write_one_attachment(&client, &att, &out_dir)
        .await
        .unwrap();

    assert_eq!(file.bytes, 11);
    let expected_path = tmp.path().join("12345").join("9876.patch.diff");
    assert!(expected_path.exists());
}

#[tokio::test]
async fn write_one_attachment_overwrites_existing_file() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let client = super::super::runtime::shared::connect_and_configure(None, None)
        .await
        .unwrap();

    let dir = tmp.path().join("12345");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("9876.patch.diff"), b"OLD CONTENT").unwrap();

    let mut att = make_attachment(9876, 12345, "patch.diff", "v2", Some(b64(b"NEW CONTENT")));
    att.size = 11;
    let out_dir = tmp.path().to_string_lossy().into_owned();

    super::write_one_attachment(&client, &att, &out_dir)
        .await
        .unwrap();

    let written = std::fs::read(dir.join("9876.patch.diff")).unwrap();
    assert_eq!(written, b"NEW CONTENT");
}

fn bug_attachments_response(bug_id: u64, atts: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "bugs": { bug_id.to_string(): atts },
    })
}

fn one_att(id: u64, bug_id: u64, file_name: &str, body: &[u8]) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "bug_id": bug_id,
        "file_name": file_name,
        "summary": file_name,
        "content_type": "text/plain",
        "creator": "dev@test.com",
        "creation_time": "2025-01-01T00:00:00Z",
        "last_change_time": "2025-01-01T00:00:00Z",
        "is_obsolete": false,
        "is_patch": false,
        "is_private": false,
        "size": body.len(),
        "data": b64(body),
    })
}

fn xmlrpc_one_att(id: u64, bug_id: u64, file_name: &str, body: &[u8]) -> String {
    format!(
        "<value><struct>\
            <member><name>id</name><value><int>{id}</int></value></member>\
            <member><name>bug_id</name><value><int>{bug_id}</int></value></member>\
            <member><name>file_name</name><value><string>{file_name}</string></value></member>\
            <member><name>summary</name><value><string>{file_name}</string></value></member>\
            <member><name>content_type</name><value><string>text/plain</string></value></member>\
            <member><name>creator</name><value><string>dev@test.com</string></value></member>\
            <member><name>creation_time</name><value><dateTime.iso8601>20250101T00:00:00</dateTime.iso8601></value></member>\
            <member><name>last_change_time</name><value><dateTime.iso8601>20250101T00:00:00</dateTime.iso8601></value></member>\
            <member><name>is_obsolete</name><value><int>0</int></value></member>\
            <member><name>is_patch</name><value><int>0</int></value></member>\
            <member><name>is_private</name><value><int>0</int></value></member>\
            <member><name>size</name><value><int>{}</int></value></member>\
            <member><name>data</name><value><base64>{}</base64></value></member>\
        </struct></value>",
        body.len(),
        b64(body),
    )
}

fn xmlrpc_bug_attachments_response(bug_id: u64, entries: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><methodResponse><params><param><value><struct>\
            <member><name>bugs</name><value><struct>\
                <member><name>{bug_id}</name><value><array><data>{entries}</data></array></value></member>\
            </struct></value></member>\
        </struct></value></param></params></methodResponse>"
    )
}

#[tokio::test]
async fn attachment_download_batch_per_bug_writes_per_bug_subdir() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/12345/attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bug_attachments_response(
                12345,
                &serde_json::json!([
                    one_att(9876, 12345, "patch.diff", b"alpha"),
                    one_att(9877, 12345, "trace.log", b"bravo charlie"),
                ]),
            )),
        )
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345],
        out: None,
        out_dir: out_dir.clone(),
    };

    let mut __io_a4 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a4.writers(),
    )
    .await;

    let output = __io_a4.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["summary"]["succeeded"], 2);
    assert_eq!(parsed["summary"]["failed"], 0);
    assert_eq!(parsed["summary"]["total_bytes"], 5 + 13);
    assert_eq!(parsed["bug_results"][0]["bug_id"], 12345);
    assert_eq!(parsed["bug_results"][0]["status"], "ok");

    assert!(tmp.path().join("12345").join("9876.patch.diff").exists());
    assert!(tmp.path().join("12345").join("9877.trace.log").exists());
    let p1 = std::fs::read(tmp.path().join("12345").join("9876.patch.diff")).unwrap();
    assert_eq!(p1, b"alpha");
}

#[tokio::test]
async fn attachment_download_batch_hybrid_uses_xmlrpc_inline_data_without_fallback() {
    let (_lock, mock, tmp) = setup_test_env().await;
    let entries = format!(
        "{}{}",
        xmlrpc_one_att(9876, 12345, "patch.diff", b"alpha"),
        xmlrpc_one_att(9877, 12345, "trace.log", b"bravo"),
    );

    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(xmlrpc_bug_attachments_response(12345, &entries)),
        )
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/9876"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/9877"))
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345],
        out: None,
        out_dir,
    };

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        Some(crate::types::ApiMode::Hybrid),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "expected ok, got {result:?}");
    assert_eq!(
        std::fs::read(tmp.path().join("12345").join("9876.patch.diff")).unwrap(),
        b"alpha",
    );
    assert_eq!(
        std::fs::read(tmp.path().join("12345").join("9877.trace.log")).unwrap(),
        b"bravo",
    );
}

#[tokio::test]
async fn attachment_download_batch_collision_filenames_resolved_by_att_id_prefix() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/12345/attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bug_attachments_response(
                12345,
                &serde_json::json!([
                    one_att(9876, 12345, "trace.log", b"first"),
                    one_att(9877, 12345, "trace.log", b"second"),
                ]),
            )),
        )
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345],
        out: None,
        out_dir,
    };

    let mut __io = crate::test_helpers::CapturedIo::new();

    let result = super::execute(&action, None, OutputFormat::Json, None, &mut __io.writers()).await;

    let _ = __io.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");

    let p1 = tmp.path().join("12345").join("9876.trace.log");
    let p2 = tmp.path().join("12345").join("9877.trace.log");
    assert_eq!(std::fs::read(&p1).unwrap(), b"first");
    assert_eq!(std::fs::read(&p2).unwrap(), b"second");
}

#[tokio::test]
async fn attachment_download_batch_mixed_bug_and_positional_ids() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/12345/attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bug_attachments_response(
                12345,
                &serde_json::json!([one_att(9876, 12345, "patch.diff", b"from bug")]),
            )),
        )
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/4242"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": {
                "4242": one_att(4242, 67890, "extra.bin", b"from positional"),
            }
        })))
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![4242],
        bug_ids: vec![12345],
        out: None,
        out_dir: out_dir.clone(),
    };

    let mut __io_a5 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a5.writers(),
    )
    .await;

    let output = __io_a5.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["summary"]["succeeded"], 2);
    assert_eq!(parsed["bug_results"][0]["bug_id"], 12345);
    assert_eq!(parsed["attachment_results"][0]["attachment_id"], 4242);
    assert_eq!(parsed["attachment_results"][0]["bug_id"], 67890);

    assert!(tmp.path().join("12345").join("9876.patch.diff").exists());
    assert!(tmp.path().join("67890").join("4242.extra.bin").exists());
}

#[tokio::test]
async fn attachment_download_batch_empty_bug_zero_files_success() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/12345/attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(bug_attachments_response(12345, &serde_json::json!([]))),
        )
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345],
        out: None,
        out_dir,
    };

    let mut __io_a6 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a6.writers(),
    )
    .await;

    let output = __io_a6.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["bug_results"][0]["status"], "ok");
    assert_eq!(parsed["summary"]["succeeded"], 0);
    assert_eq!(parsed["summary"]["failed"], 0);
}

#[tokio::test]
async fn attachment_download_batch_legacy_single_id_unchanged() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/9876"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": {
                "9876": one_att(9876, 12345, "patch.diff", b"legacy"),
            }
        })))
        .mount(&mock)
        .await;

    let out_path = tmp.path().join("downloaded.bin");
    let action = AttachmentAction::Download {
        ids: vec![9876],
        bug_ids: vec![],
        out: Some(out_path.to_string_lossy().into_owned()),
        out_dir: "./attachments".into(),
    };

    let mut __io_a7 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a7.writers(),
    )
    .await;

    let output = __io_a7.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");

    // Legacy path emits DownloadResult, not AttachmentBatchResult — the
    // JSON has `id` at the top level, not `summary`.
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["id"], 9876);
    assert_eq!(parsed["size"].as_u64().unwrap_or(0), 6);
    assert!(out_path.exists());
    assert_eq!(std::fs::read(&out_path).unwrap(), b"legacy");
}

#[tokio::test]
async fn attachment_download_single_out_dash_streams_bytes_without_result() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/9876"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": {
                "9876": one_att(9876, 12345, "trace.bin", b"raw\0bytes\n"),
            }
        })))
        .mount(&mock)
        .await;

    let dash_path = std::path::Path::new("-");
    assert!(
        !dash_path.exists(),
        "stdout-mode test requires no pre-existing file named '-' in the cwd",
    );

    let action = AttachmentAction::Download {
        ids: vec![9876],
        bug_ids: vec![],
        out: Some("-".into()),
        out_dir: "./attachments".into(),
    };
    let mut io = crate::test_helpers::CapturedIo::new();

    let result = super::execute(&action, None, OutputFormat::Json, None, &mut io.writers()).await;

    let wrote_dash_file = dash_path.exists();
    if wrote_dash_file {
        std::fs::remove_file(dash_path).unwrap();
    }

    assert!(result.is_ok(), "expected stdout download, got {result:?}");
    assert_eq!(io.out, b"raw\0bytes\n");
    assert!(
        io.err.is_empty(),
        "stderr should stay empty: {:?}",
        io.err_str()
    );
    assert!(
        !wrote_dash_file,
        "--out - must stream to stdout, not create a file named '-'",
    );
}

#[tokio::test]
async fn attachment_download_batch_bug_not_found_partial_failure() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/12345/attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bug_attachments_response(
                12345,
                &serde_json::json!([one_att(9876, 12345, "patch.diff", b"ok")]),
            )),
        )
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/99999/attachment"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": true,
            "code": 101,
            "message": "Bug 99999 does not exist."
        })))
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345, 99999],
        out: None,
        out_dir,
    };

    let mut __io_a8 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a8.writers(),
    )
    .await;

    let output = __io_a8.out_str().to_string();
    assert!(result.is_err(), "expected BatchPartialFailure");
    let err = result.unwrap_err();
    let exit = err.exit_code();
    assert_eq!(exit, 11, "expected exit 11, got {exit}: {err}");

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["summary"]["succeeded"], 1);
    assert_eq!(parsed["summary"]["failed"], 1);
    let bugs = parsed["bug_results"].as_array().unwrap();
    assert_eq!(bugs[0]["bug_id"], 12345);
    assert_eq!(bugs[0]["status"], "ok");
    assert_eq!(bugs[1]["bug_id"], 99999);
    assert_eq!(bugs[1]["status"], "error");
}

#[tokio::test]
async fn attachment_download_batch_all_targets_fail_still_exit_11() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/99999/attachment"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": true,
            "code": 101,
            "message": "Bug 99999 does not exist."
        })))
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![99999],
        out: None,
        out_dir,
    };
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "expected BatchPartialFailure");
    let err = result.unwrap_err();
    let exit = err.exit_code();
    assert_eq!(exit, 11, "all-fail still uses BatchPartialFailure: {err}");
}

#[tokio::test]
async fn attachment_download_batch_obsolete_attachments_included() {
    let (_lock, mock, tmp) = setup_test_env().await;

    let mut obsolete = one_att(9876, 12345, "old.patch", b"obsolete content");
    obsolete["is_obsolete"] = serde_json::json!(true);
    Mock::given(method("GET"))
        .and(path("/rest/bug/12345/attachment"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(bug_attachments_response(
                12345,
                &serde_json::json!([obsolete]),
            )),
        )
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345],
        out: None,
        out_dir: out_dir.clone(),
    };

    let mut __io2 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io2.writers(),
    )
    .await;

    let _ = __io2.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");
    assert!(tmp.path().join("12345").join("9876.old.patch").exists());
}

#[tokio::test]
async fn attachment_download_batch_data_missing_falls_back_via_get() {
    let (_lock, mock, tmp) = setup_test_env().await;

    // Listing returns the attachment metadata WITHOUT data.
    let mut att = one_att(9876, 12345, "patch.diff", b"x");
    att["data"] = serde_json::Value::Null;
    Mock::given(method("GET"))
        .and(path("/rest/bug/12345/attachment"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(bug_attachments_response(12345, &serde_json::json!([att]))),
        )
        .mount(&mock)
        .await;

    // Fallback fetch DOES return data.
    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/9876"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": {
                "9876": one_att(9876, 12345, "patch.diff", b"fallback"),
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345],
        out: None,
        out_dir,
    };

    let mut __io3 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io3.writers(),
    )
    .await;

    let _ = __io3.out_str().to_string();
    assert!(result.is_ok(), "expected ok, got {result:?}");
    let written = std::fs::read(tmp.path().join("12345").join("9876.patch.diff")).unwrap();
    assert_eq!(written, b"fallback");
}

#[cfg(unix)]
#[tokio::test]
async fn attachment_download_batch_top_level_out_dir_unwritable_fails_fast() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    // /dev/null/attachments — create_dir_all on a path under /dev/null
    // (which is a character device, not a directory) → ENOTDIR.
    let action = AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![12345],
        out: None,
        out_dir: "/dev/null/attachments".into(),
    };
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "expected Io");
    let err = result.unwrap_err();
    let exit = err.exit_code();
    assert_eq!(exit, 6, "expected Io exit 6 (fail-fast), got {exit}: {err}");
}

#[tokio::test]
async fn write_one_attachment_invalid_base64_returns_data_integrity() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let client = super::super::runtime::shared::connect_and_configure(None, None)
        .await
        .unwrap();

    let mut att = make_attachment(
        9876,
        12345,
        "patch.diff",
        "broken",
        Some("not valid base64 !!".into()),
    );
    att.size = 0;
    let out_dir = tmp.path().to_string_lossy().into_owned();

    let result = super::write_one_attachment(&client, &att, &out_dir).await;
    assert!(result.is_err(), "expected DataIntegrity for invalid base64");
    let err = result.unwrap_err();
    assert!(
        matches!(err, crate::error::BzrError::DataIntegrity(_)),
        "expected DataIntegrity, got {err}",
    );
    let msg = err.to_string();
    assert!(
        msg.contains("decode attachment #9876"),
        "expected decode error message including att-id, got: {msg}",
    );
}

// ── filename sanitization (path traversal) ──────────────────────────

#[test]
fn safe_basename_strips_directory_components() {
    assert_eq!(super::safe_basename("normal.txt").unwrap(), "normal.txt");
    assert_eq!(super::safe_basename("../../etc/passwd").unwrap(), "passwd");
    assert_eq!(super::safe_basename("/etc/passwd").unwrap(), "passwd");
    assert_eq!(super::safe_basename("a/b/c.diff").unwrap(), "c.diff");
}

#[test]
fn safe_basename_rejects_names_without_a_basename() {
    assert!(super::safe_basename("").is_err());
    assert!(super::safe_basename("..").is_err());
    assert!(super::safe_basename(".").is_err());
    assert!(super::safe_basename("foo/..").is_err());
}

#[test]
fn single_download_dest_honors_explicit_out_verbatim() {
    let dest = super::single_download_dest(Some("/tmp/user/chosen.bin"), "server.txt").unwrap();
    assert_eq!(dest, std::path::Path::new("/tmp/user/chosen.bin"));
}

#[test]
fn single_download_dest_sanitizes_server_filename_when_no_out() {
    let dest = super::single_download_dest(None, "../../escape.txt").unwrap();
    assert_eq!(dest, std::path::Path::new("escape.txt"));
}

#[tokio::test]
async fn write_one_attachment_sanitizes_server_filename_with_separators() {
    let (_lock, _mock, tmp) = setup_test_env().await;
    let client = super::super::runtime::shared::connect_and_configure(None, None)
        .await
        .unwrap();

    let att = make_attachment(7, 42, "sub/dir/escape.txt", "evil", Some(b64(b"data")));
    let out_dir = tmp.path().to_string_lossy().into_owned();

    let file = super::write_one_attachment(&client, &att, &out_dir)
        .await
        .unwrap();

    let expected = tmp.path().join("42").join("7.escape.txt");
    assert_eq!(std::path::Path::new(&file.path), expected);
    assert!(expected.exists(), "{expected:?} not found");
}

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
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut __io.writers()).await;
    assert!(result.is_ok(), "view should succeed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(__io.out_str().trim()).unwrap();
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
    let result = super::execute(
        &action,
        None,
        OutputFormat::Table,
        None,
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
