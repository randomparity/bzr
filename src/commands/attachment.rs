use std::path::Path;

use crate::cli::AttachmentAction;
use crate::error::Result;
use crate::output::{self, ActionResult, DownloadResult, ResourceKind, UploadResult};
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{UpdateAttachmentParams, UploadAttachmentParams};

pub async fn execute(
    action: &AttachmentAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_client(server, api).await?;

    match action {
        AttachmentAction::List { bug_id } => {
            let attachments = client.get_attachments(*bug_id).await?;
            output::print_attachments(&attachments, format);
        }
        AttachmentAction::Download { id, out } => {
            let (filename, data) = client.download_attachment(*id).await?;
            let dest = out.as_deref().unwrap_or(&filename);
            std::fs::write(dest, &data)?;
            output::print_result(
                &DownloadResult::new(*id, dest, data.len()),
                &format!(
                    "Downloaded attachment #{id} to {dest} ({} bytes)",
                    data.len()
                ),
                format,
            );
        }
        AttachmentAction::Upload {
            bug_id,
            file,
            summary,
            content_type,
            flag,
        } => {
            let path = Path::new(file);
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or(file);
            let data = std::fs::read(path)?;
            let summary = summary.as_deref().unwrap_or(file_name);
            let ct = content_type
                .as_deref()
                .unwrap_or_else(|| guess_content_type(file_name));
            let flags = super::flags::parse_flags(flag)?;
            let size = data.len();
            let upload_params = UploadAttachmentParams {
                bug_id: *bug_id,
                file_name: file_name.to_string(),
                summary: summary.to_string(),
                content_type: ct.to_string(),
                data,
                flags,
            };
            let att_id = client.upload_attachment(&upload_params).await?;
            output::print_result(
                &UploadResult::new(att_id, *bug_id, size),
                &format!("Uploaded attachment #{att_id} to bug #{bug_id} ({size} bytes)",),
                format,
            );
        }
        AttachmentAction::Update {
            id,
            summary,
            file_name,
            content_type,
            obsolete,
            is_patch,
            is_private,
            flag,
        } => {
            let flags = super::flags::parse_flags(flag)?;
            let params = UpdateAttachmentParams {
                summary: summary.clone(),
                file_name: file_name.clone(),
                content_type: content_type.clone(),
                is_obsolete: *obsolete,
                is_patch: *is_patch,
                is_private: *is_private,
                flags,
            };
            client.update_attachment(*id, &params).await?;
            output::print_result(
                &ActionResult::updated(*id, ResourceKind::Attachment),
                &format!("Updated attachment #{id}"),
                format,
            );
        }
    }
    Ok(())
}

fn guess_content_type(filename: &str) -> &'static str {
    match filename
        .rsplit('.')
        .next()
        .map(str::to_lowercase)
        .as_deref()
    {
        Some(
            "txt" | "log" | "c" | "h" | "cpp" | "rs" | "py" | "sh" | "pl" | "rb" | "js" | "ts",
        ) => "text/plain",
        Some("html" | "htm") => "text/html",
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("gz" | "tgz") => "application/gzip",
        Some("zip") => "application/zip",
        Some("tar") => "application/x-tar",
        Some("patch" | "diff") => "text/x-diff",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    use super::super::test_helpers::{capture_stdout, extract_json, setup_test_env};
    use crate::cli::AttachmentAction;
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
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed = extract_json(&output);
        assert_eq!(parsed[0]["id"], 100);
        assert_eq!(parsed[0]["file_name"], "patch.diff");
        assert_eq!(parsed[0]["creator"], "dev@test.com");
    }

    #[tokio::test]
    async fn attachment_list_api_error_propagates() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug/999/attachment"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock)
            .await;

        let action = AttachmentAction::List { bug_id: 999 };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn attachment_upload_api_error_propagates() {
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

        let action = AttachmentAction::Upload {
            bug_id: 42,
            file: upload_file.to_string_lossy().into_owned(),
            summary: Some("Test".into()),
            content_type: None,
            flag: vec![],
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("cannot attach"),
            "expected API error message, got: {err}"
        );
    }

    #[tokio::test]
    async fn attachment_download_api_error_propagates() {
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

        let action = AttachmentAction::Download { id: 404, out: None };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn attachment_upload_returns_id() {
        let (_lock, mock, tmp) = setup_test_env().await;

        Mock::given(method("POST"))
            .and(path("/rest/bug/42/attachment"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [200]})),
            )
            .mount(&mock)
            .await;

        let upload_file = tmp.path().join("upload.txt");
        std::fs::write(&upload_file, "test content").unwrap();

        let action = AttachmentAction::Upload {
            bug_id: 42,
            file: upload_file.to_string_lossy().into_owned(),
            summary: Some("Test upload".into()),
            content_type: Some("text/plain".into()),
            flag: vec![],
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed = extract_json(&output);
        assert_eq!(parsed["id"], 200);
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

        let action = AttachmentAction::Update {
            id: 99,
            summary: Some("Updated summary".into()),
            file_name: None,
            content_type: None,
            obsolete: None,
            is_patch: None,
            is_private: None,
            flag: vec![],
        };
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed = extract_json(&output);
        assert_eq!(parsed["id"], 99);
        assert_eq!(parsed["action"], "updated");
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
}
