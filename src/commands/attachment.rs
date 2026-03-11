use std::path::Path;

use crate::cli::AttachmentAction;
use crate::client::BugzillaClient;
use crate::config::Config;
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn execute(
    action: &AttachmentAction,
    server: Option<&str>,
    format: &OutputFormat,
) -> Result<()> {
    let config = Config::load()?;
    let srv = config.active_server(server)?;
    let client = BugzillaClient::new(&srv.url, &srv.api_key)?;

    match action {
        AttachmentAction::List { bug_id } => {
            let attachments = client.get_attachments(*bug_id).await?;
            output::print_attachments(&attachments, format)?;
        }
        AttachmentAction::Download { id, output: out } => {
            let (filename, data) = client.download_attachment(*id).await?;
            let dest = out.as_deref().unwrap_or(&filename);
            std::fs::write(dest, &data)?;
            println!(
                "Downloaded attachment #{} to {} ({} bytes)",
                id,
                dest,
                data.len()
            );
        }
        AttachmentAction::Upload {
            bug_id,
            file,
            summary,
            content_type,
        } => {
            let path = Path::new(file);
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or(file);
            let data = std::fs::read(path)?;
            let summary = summary.as_deref().unwrap_or(file_name);
            let ct = content_type
                .as_deref()
                .unwrap_or_else(|| guess_content_type(file_name));
            let att_id = client
                .upload_attachment(*bug_id, file_name, summary, ct, &data)
                .await?;
            println!(
                "Uploaded attachment #{} to bug #{} ({} bytes)",
                att_id,
                bug_id,
                data.len()
            );
        }
    }
    Ok(())
}

fn guess_content_type(filename: &str) -> &'static str {
    match filename
        .rsplit('.')
        .next()
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("txt" | "log") => "text/plain",
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
        Some("c" | "h" | "cpp" | "rs" | "py" | "sh" | "pl" | "rb" | "js" | "ts") => "text/plain",
        _ => "application/octet-stream",
    }
}
