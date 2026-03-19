use std::path::Path;

use crate::cli::AttachmentAction;
use crate::config::ApiMode;
use crate::error::Result;
use crate::output::{self, OutputFormat};
use crate::types::{UpdateAttachmentParams, UploadAttachmentParams};

pub async fn execute(
    action: &AttachmentAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::build_client(server, api).await?;

    match action {
        AttachmentAction::List { bug_id } => {
            let attachments = client.get_attachments(*bug_id).await?;
            output::print_attachments(&attachments, format);
        }
        AttachmentAction::Download { id, output: out } => {
            let (filename, data) = client.download_attachment(*id).await?;
            let dest = out.as_deref().unwrap_or(&filename);
            std::fs::write(dest, &data)?;
            output::print_result(
                &serde_json::json!({
                    "id": id,
                    "file": dest,
                    "size": data.len(),
                    "resource": "attachment",
                    "action": "downloaded",
                }),
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
            let flags = super::shared::parse_flags(flag)?;
            let upload_params = UploadAttachmentParams {
                bug_id: *bug_id,
                file_name: file_name.to_string(),
                summary: summary.to_string(),
                content_type: ct.to_string(),
                data: data.clone(),
                flags,
            };
            let att_id = client.upload_attachment(&upload_params).await?;
            output::print_result(
                &serde_json::json!({
                    "id": att_id,
                    "bug_id": bug_id,
                    "size": data.len(),
                    "resource": "attachment",
                    "action": "created",
                }),
                &format!(
                    "Uploaded attachment #{att_id} to bug #{bug_id} ({} bytes)",
                    data.len()
                ),
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
            let flags = super::shared::parse_flags(flag)?;
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
                &serde_json::json!({"id": id, "resource": "attachment", "action": "updated"}),
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
mod tests {
    use super::*;

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
