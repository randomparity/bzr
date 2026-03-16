use std::path::Path;

use crate::cli::AttachmentAction;
use crate::client::UpdateAttachmentParams;
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn execute(
    action: &AttachmentAction,
    server: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let client = super::shared::build_client(server).await?;

    match action {
        AttachmentAction::List { bug_id } => {
            let attachments = client.get_attachments(*bug_id).await?;
            output::print_attachments(&attachments, format);
        }
        AttachmentAction::Download { id, output: out } => {
            let (filename, data) = client.download_attachment(*id).await?;
            let dest = out.as_deref().unwrap_or(&filename);
            std::fs::write(dest, &data)?;
            #[expect(clippy::print_stdout)]
            {
                println!(
                    "Downloaded attachment #{} to {} ({} bytes)",
                    id,
                    dest,
                    data.len()
                );
            }
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
            let att_id = if flags.is_empty() {
                client
                    .upload_attachment(*bug_id, file_name, summary, ct, &data)
                    .await?
            } else {
                client
                    .upload_attachment_with_flags(*bug_id, file_name, summary, ct, &data, &flags)
                    .await?
            };
            #[expect(clippy::print_stdout)]
            {
                println!(
                    "Uploaded attachment #{} to bug #{} ({} bytes)",
                    att_id,
                    bug_id,
                    data.len()
                );
            }
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
            #[expect(clippy::print_stdout)]
            {
                println!("Updated attachment #{id}");
            }
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
