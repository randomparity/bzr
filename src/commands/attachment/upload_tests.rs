#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{AttachmentAction, UploadArgs};
use crate::test_helpers::{setup_empty_config_env, setup_test_env};
use crate::types::OutputFormat;

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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
async fn attachment_upload_missing_source_names_role_and_path() {
    let mut io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, tmp) = setup_test_env().await;
    let missing = tmp.path().join("missing-upload.txt");
    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: missing.to_string_lossy().into_owned(),
        summary: None,
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

    let err = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err()
    .to_string();
    let missing = missing.to_string_lossy();

    assert!(
        err.contains("read attachment upload file") && err.contains(missing.as_ref()),
        "error should name upload role and path, got: {err}"
    );
}

#[tokio::test]
async fn attachment_upload_missing_source_fails_before_connect() {
    let (_lock, tmp) = setup_empty_config_env().await;
    let missing = tmp.path().join("missing-upload.txt");
    let action = AttachmentAction::Upload(UploadArgs {
        bug_id: 42,
        file: missing.to_string_lossy().into_owned(),
        summary: None,
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

    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err()
    .to_string();
    let missing = missing.to_string_lossy();

    assert!(
        err.contains("read attachment upload file") && err.contains(missing.as_ref()),
        "local upload file error should win over config lookup, got: {err}"
    );
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "upload --patch should succeed: {result:?}");
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "upload --patch with explicit ct should succeed: {result:?}"
    );
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "step-2 failure should propagate");
    // The partial-failure warning must name the uploaded attachment so the
    // user knows the upload itself succeeded (no warning => silent partial loss).
    let err_out = __cap_io.err_str();
    assert!(
        err_out.contains("attachment #200") && err_out.contains("privacy flip failed"),
        "expected partial-failure warning naming attachment #200, got: {err_out:?}"
    );
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(matches!(
        result,
        Err(crate::error::BzrError::DataIntegrity(_))
    ));
}

#[test]
fn guess_content_type_text_plain() {
    assert_eq!(super::guess_content_type("file.txt"), "text/plain");
    assert_eq!(super::guess_content_type("script.py"), "text/plain");
    assert_eq!(super::guess_content_type("main.rs"), "text/plain");
    assert_eq!(super::guess_content_type("code.c"), "text/plain");
    assert_eq!(super::guess_content_type("app.js"), "text/plain");
}

#[test]
fn guess_content_type_html() {
    assert_eq!(super::guess_content_type("page.html"), "text/html");
    assert_eq!(super::guess_content_type("page.htm"), "text/html");
}

#[test]
fn guess_content_type_json() {
    assert_eq!(super::guess_content_type("data.json"), "application/json");
}

#[test]
fn guess_content_type_structured_documents() {
    assert_eq!(super::guess_content_type("config.xml"), "application/xml");
    assert_eq!(super::guess_content_type("report.pdf"), "application/pdf");
}

#[test]
fn guess_content_type_images() {
    assert_eq!(super::guess_content_type("photo.png"), "image/png");
    assert_eq!(super::guess_content_type("photo.jpg"), "image/jpeg");
    assert_eq!(super::guess_content_type("photo.jpeg"), "image/jpeg");
    assert_eq!(super::guess_content_type("anim.gif"), "image/gif");
    assert_eq!(super::guess_content_type("logo.svg"), "image/svg+xml");
}

#[test]
fn guess_content_type_archives() {
    assert_eq!(super::guess_content_type("file.gz"), "application/gzip");
    assert_eq!(super::guess_content_type("file.tgz"), "application/gzip");
    assert_eq!(super::guess_content_type("file.zip"), "application/zip");
    assert_eq!(super::guess_content_type("file.tar"), "application/x-tar");
}

#[test]
fn guess_content_type_diff() {
    assert_eq!(super::guess_content_type("fix.patch"), "text/x-diff");
    assert_eq!(super::guess_content_type("changes.diff"), "text/x-diff");
}

#[test]
fn guess_content_type_unknown() {
    assert_eq!(
        super::guess_content_type("file.xyz"),
        "application/octet-stream"
    );
    assert_eq!(
        super::guess_content_type("noext"),
        "application/octet-stream"
    );
    assert_eq!(
        super::guess_content_type(".env"),
        "application/octet-stream"
    );
}

#[test]
fn guess_content_type_case_insensitive() {
    assert_eq!(super::guess_content_type("FILE.TXT"), "text/plain");
    assert_eq!(super::guess_content_type("image.PNG"), "image/png");
    assert_eq!(super::guess_content_type("data.JSON"), "application/json");
}
