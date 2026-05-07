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
    let client = super::shared::connect_and_configure(server, api).await?;

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
            private,
            is_patch,
            comment,
            flag,
        } => {
            let path = Path::new(file);
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or(file);
            let data = std::fs::read(path)?;
            let summary = summary.as_deref().unwrap_or(file_name);
            let ct = match (content_type.as_deref(), *is_patch) {
                (Some(explicit), _) => explicit.to_string(),
                (None, true) => "text/plain".to_string(),
                (None, false) => guess_content_type(file_name).to_string(),
            };
            let flags = super::flags::parse_flags(flag)?;
            let size = data.len();
            let upload_params = UploadAttachmentParams {
                bug_id: *bug_id,
                file_name: file_name.to_string(),
                summary: summary.to_string(),
                content_type: ct,
                data,
                flags,
                is_private: *private,
                comment: comment.clone(),
                is_patch: *is_patch,
            };
            let att_id = client.upload_attachment(&upload_params).await?;
            output::print_result(
                &UploadResult::new(att_id, *bug_id, size),
                &format!("Uploaded attachment #{att_id} to bug #{bug_id} ({size} bytes)"),
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
#[path = "attachment_tests.rs"]
mod tests;
