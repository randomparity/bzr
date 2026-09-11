#![expect(clippy::unwrap_used, clippy::panic)]

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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a2.writers(),
    )
    .await;
    let output = __io_a2.out_str().to_string();
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(&output);
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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(matches!(
        result,
        Err(crate::error::BzrError::InputValidation { .. })
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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(matches!(
        result,
        Err(crate::error::BzrError::InputValidation { .. })
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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(matches!(
        result,
        Err(crate::error::BzrError::InputValidation { .. })
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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
        bug_ids: vec![42],
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(matches!(
        result,
        Err(crate::error::BzrError::DataIntegrity(_))
    ));
}

#[tokio::test]
async fn upload_single_bug_keeps_the_upload_result_shape() {
    let mut __io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/1/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [11]})))
        .mount(&mock)
        .await;

    let upload_file = tmp.path().join("upload.txt");
    std::fs::write(&upload_file, "test content").unwrap();

    let action = AttachmentAction::Upload(UploadArgs {
        bug_ids: vec![1],
        file: upload_file.to_string_lossy().into_owned(),
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "single-bug upload should succeed: {result:?}"
    );
    let output = __io.out_str().to_string();
    let data = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(data["bug_id"], 1);
    assert!(
        data.get("uploaded").is_none(),
        "single-bug upload must not gain the batch shape: {data:?}"
    );
}

#[tokio::test]
async fn upload_fans_out_to_every_bug() {
    let mut __io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/1/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [11]})))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/bug/2/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [22]})))
        .mount(&mock)
        .await;

    let upload_file = tmp.path().join("upload.txt");
    std::fs::write(&upload_file, "test content").unwrap();

    let action = AttachmentAction::Upload(UploadArgs {
        bug_ids: vec![1, 2],
        file: upload_file.to_string_lossy().into_owned(),
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(result.is_ok(), "fan-out should succeed: {result:?}");
    let output = __io.out_str().to_string();
    let data = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(
        data["uploaded"],
        serde_json::json!([
            {"bug_id": 1, "attachment_id": 11},
            {"bug_id": 2, "attachment_id": 22},
        ])
    );
    assert_eq!(data["failed"], serde_json::json!([]));
}

#[tokio::test]
async fn upload_partial_failure_records_both_outcomes() {
    let mut __io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/1/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [11]})))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/bug/2/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "error": true,
            "code": 100,
            "message": "Bug 2 does not exist."
        })))
        .mount(&mock)
        .await;

    let upload_file = tmp.path().join("upload.txt");
    std::fs::write(&upload_file, "test content").unwrap();

    let action = AttachmentAction::Upload(UploadArgs {
        bug_ids: vec![1, 2],
        file: upload_file.to_string_lossy().into_owned(),
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    match result {
        Err(crate::error::BzrError::BatchPartialFailure { succeeded, failed }) => {
            assert_eq!(succeeded, 1);
            assert_eq!(failed, 1);
        }
        other => panic!("expected BatchPartialFailure, got {other:?}"),
    }
    let output = __io.out_str().to_string();
    let data = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(
        data["uploaded"],
        serde_json::json!([{"bug_id": 1, "attachment_id": 11}])
    );
    let failed = data["failed"].as_array().unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["bug_id"], 2);
    assert!(failed[0].get("step").is_none());
}

#[tokio::test]
async fn upload_comment_private_failure_is_a_sub_step() {
    let mut __io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("p.diff");
    std::fs::write(&upload_file, "diff --git a b").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/bug/1/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [11]})))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "1": {
                    "comments": [
                        {"id": 101, "bug_id": 1, "text": "sensitive", "is_private": false, "attachment_id": 11}
                    ]
                }
            }
        })))
        .mount(&mock)
        .await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/2/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [22]})))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/2/comment"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden: editbugs required"))
        .mount(&mock)
        .await;

    let action = AttachmentAction::Upload(UploadArgs {
        bug_ids: vec![1, 2],
        file: upload_file.to_string_lossy().into_owned(),
        summary: None,
        content_type: None,
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    match result {
        Err(crate::error::BzrError::BatchPartialFailure { succeeded, failed }) => {
            // Bug 2 received the attachment (it is still in `uploaded` below)
            // but did not fully succeed, so it counts once, on the failed
            // side — not on both, and not on neither.
            assert_eq!(succeeded, 1);
            assert_eq!(failed, 1);
        }
        other => panic!("expected BatchPartialFailure, got {other:?}"),
    }
    let output = __io.out_str().to_string();
    let data = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(
        data["uploaded"],
        serde_json::json!([
            {"bug_id": 1, "attachment_id": 11},
            {"bug_id": 2, "attachment_id": 22},
        ]),
        "bug 2 must still appear in uploaded: the attachment was created"
    );
    let failed = data["failed"].as_array().unwrap();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0]["bug_id"], 2);
    assert_eq!(failed[0]["step"], "comment_private");
}

/// The first `comment_private` failure stops the fan-out: a flip failure is
/// almost always a credential property, so it recurs on every remaining
/// target, and continuing would post the same private comment publicly on
/// each of them. Bug 1's flip fails and bug 1 keeps its `comment_private`
/// entry; bug 2 must never be contacted at all — no mock is mounted for it,
/// so if `upload_batch` wrongly kept looping, bug 2's request would 404 and
/// surface as an ordinary (stepless) failure instead of `not_attempted`,
/// which the assertions below would catch. Also a direct check of the
/// invariant `succeeded + failed == bug_ids.len()` for this shape.
#[tokio::test]
async fn upload_batch_stops_after_first_flip_failure_and_marks_the_rest_not_attempted() {
    let mut __io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("p.diff");
    std::fs::write(&upload_file, "diff --git a b").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/bug/1/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [11]})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1/comment"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden: editbugs required"))
        .expect(1)
        .mount(&mock)
        .await;

    let action = AttachmentAction::Upload(UploadArgs {
        bug_ids: vec![1, 2, 3],
        file: upload_file.to_string_lossy().into_owned(),
        summary: None,
        content_type: None,
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    match result {
        Err(crate::error::BzrError::BatchPartialFailure { succeeded, failed }) => {
            assert_eq!(succeeded + failed, 3, "counts must sum to the target count");
            assert_eq!(succeeded, 0);
            assert_eq!(failed, 3);
        }
        other => panic!("expected BatchPartialFailure, got {other:?}"),
    }
    let output = __io.out_str().to_string();
    let data = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(
        data["uploaded"],
        serde_json::json!([{"bug_id": 1, "attachment_id": 11}]),
        "only bug 1 was attempted before the stop"
    );
    let failed = data["failed"].as_array().unwrap();
    assert_eq!(failed.len(), 3);
    assert_eq!(failed[0]["bug_id"], 1);
    assert_eq!(failed[0]["step"], "comment_private");
    for (idx, bug_id) in [2u64, 3].into_iter().enumerate() {
        let entry = &failed[idx + 1];
        assert_eq!(entry["bug_id"], bug_id);
        assert_eq!(entry["step"], "not_attempted");
        assert!(
            entry["error"].as_str().unwrap().contains("bug #1"),
            "not_attempted error should name the blocking bug, got: {entry:?}"
        );
    }
}

#[tokio::test]
async fn upload_batch_table_mode_labels_not_attempted_distinctly() {
    let mut __io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("p.diff");
    std::fs::write(&upload_file, "diff --git a b").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/bug/1/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [11]})))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1/comment"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden: editbugs required"))
        .mount(&mock)
        .await;

    let action = AttachmentAction::Upload(UploadArgs {
        bug_ids: vec![1, 2],
        file: upload_file.to_string_lossy().into_owned(),
        summary: None,
        content_type: None,
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None),
        &mut __io.writers(),
    )
    .await;
    assert!(result.is_err(), "expected BatchPartialFailure");

    let err_out = __io.err_str();
    assert!(
        !err_out.contains("Failed to upload to bug #2"),
        "bug 2 was never attempted; it must not be narrated as an upload \
         failure, got: {err_out:?}"
    );
    assert!(
        err_out.contains("Not attempted for bug #2") && err_out.contains("bug #1"),
        "expected a distinct not_attempted line naming the blocking bug, got: {err_out:?}"
    );
}

/// The gate reuses `confirm_batch`'s threshold and non-TTY bypass
/// (`should_prompt` returns false off a TTY), so a batch above
/// `BATCH_THRESHOLD` run the way every functional test runs -- with no
/// controlling terminal -- must proceed exactly as before rather than
/// blocking on a prompt that can never be answered.
#[tokio::test]
async fn upload_batch_above_threshold_proceeds_without_a_controlling_tty() {
    let mut __io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("upload.txt");
    std::fs::write(&upload_file, "test content").unwrap();

    let bug_ids: Vec<u64> = (1..=11).collect();
    for &bug_id in &bug_ids {
        Mock::given(method("POST"))
            .and(path(format!("/rest/bug/{bug_id}/attachment")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [bug_id]})),
            )
            .mount(&mock)
            .await;
    }

    let action = AttachmentAction::Upload(UploadArgs {
        bug_ids: bug_ids.clone(),
        file: upload_file.to_string_lossy().into_owned(),
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
    let result = crate::commands::attachment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "an 11-bug batch must not block on an unanswerable prompt: {result:?}"
    );
    assert!(
        !__io.err_str().contains("Aborted"),
        "no prompt should fire without a controlling TTY"
    );
    let output = __io.out_str().to_string();
    let data = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(data["uploaded"].as_array().unwrap().len(), 11);
}

/// `--yes` (`ctx.assume_yes()`) must reach `upload_batch`'s confirmation
/// gate: this is the same 11-bug batch as above but with `assume_yes` set,
/// proving the flag is actually threaded through rather than only working by
/// accident of the test harness having no TTY.
#[tokio::test]
async fn upload_batch_above_threshold_with_assume_yes_bypasses_the_gate() {
    let mut __io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("upload.txt");
    std::fs::write(&upload_file, "test content").unwrap();

    let bug_ids: Vec<u64> = (1..=11).collect();
    for &bug_id in &bug_ids {
        Mock::given(method("POST"))
            .and(path(format!("/rest/bug/{bug_id}/attachment")))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [bug_id]})),
            )
            .mount(&mock)
            .await;
    }

    let action = AttachmentAction::Upload(UploadArgs {
        bug_ids: bug_ids.clone(),
        file: upload_file.to_string_lossy().into_owned(),
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
    let ctx =
        crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_assume_yes(true);
    let result = crate::commands::attachment::execute(&action, &ctx, &mut __io.writers()).await;
    assert!(
        result.is_ok(),
        "assume_yes should bypass the gate: {result:?}"
    );
    let output = __io.out_str().to_string();
    let data = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(data["uploaded"].as_array().unwrap().len(), 11);
}

#[tokio::test]
async fn upload_batch_table_mode_labels_a_sub_step_failure() {
    let mut __io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("p.diff");
    std::fs::write(&upload_file, "diff --git a b").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/bug/1/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [11]})))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "1": {
                    "comments": [
                        {"id": 101, "bug_id": 1, "text": "sensitive", "is_private": false, "attachment_id": 11}
                    ]
                }
            }
        })))
        .mount(&mock)
        .await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&mock)
        .await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/2/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [22]})))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/2/comment"))
        .respond_with(ResponseTemplate::new(403).set_body_string("Forbidden: editbugs required"))
        .mount(&mock)
        .await;

    let action = AttachmentAction::Upload(UploadArgs {
        bug_ids: vec![1, 2],
        file: upload_file.to_string_lossy().into_owned(),
        summary: None,
        content_type: None,
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
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None),
        &mut __io.writers(),
    )
    .await;
    assert!(result.is_err(), "expected BatchPartialFailure");

    let err_out = __io.err_str();
    assert!(
        !err_out.contains("Failed to upload to bug #2"),
        "bug 2's attachment was created; it must not be narrated as an upload \
         failure, got: {err_out:?}"
    );
    assert_eq!(
        err_out.lines().count(),
        1,
        "expected exactly one stderr line for the sub-step failure, got: {err_out:?}"
    );
    assert!(
        err_out.contains("bug #2") && err_out.contains("could not make the comment private"),
        "expected a line naming the uploaded-but-not-private outcome, got: {err_out:?}"
    );
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

#[test]
fn write_batch_upload_table_escapes_per_item_errors() {
    use crate::output::result_types::{BatchUploadResult, UploadFailure};
    use crate::test_helpers::CapturedIo;

    // All three per-item stderr arms carry a `BzrError` display that can embed
    // server-supplied text, so each escapes its own interpolation (ADR 0070).
    let result = BatchUploadResult::new(
        0,
        vec![],
        vec![
            UploadFailure::new(1, "boom\u{1b}[2J"),
            UploadFailure::comment_private(2, "flip\u{202e}tail"),
            UploadFailure::not_attempted(3, 2),
        ],
    );
    let mut io = CapturedIo::new();

    super::write_batch_upload(&result, OutputFormat::Table, &mut io.writers());

    let err = io.err_str();
    assert!(
        err.contains("Failed to upload to bug #1: boom\\u{1b}[2J"),
        "plain failure arm: {err:?}"
    );
    assert!(
        err.contains("could not make the comment private: flip\\u{202e}tail"),
        "comment_private arm: {err:?}"
    );
    assert!(
        err.contains("Not attempted for bug #3:"),
        "not_attempted arm still renders: {err:?}"
    );
    assert!(
        !err.contains('\u{1b}') && !err.contains('\u{202e}'),
        "no raw control may reach stderr: {err:?}"
    );
}
