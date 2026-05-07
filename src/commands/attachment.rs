use std::collections::HashMap;
use std::io::Write as _;
use std::path::Path;

use crate::cli::AttachmentAction;
use crate::client::BugzillaClient;
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
    validate_action(action)?;
    let client = super::shared::connect_and_configure(server, api).await?;

    match action {
        AttachmentAction::List { bug_id } => {
            let attachments = client.get_attachments(*bug_id).await?;
            output::print_attachments(&attachments, format);
        }
        AttachmentAction::Download {
            ids,
            bug_ids,
            out,
            out_dir: _,
        } => {
            // Bulk dispatch (multi-ID and/or `--bug`) lands in Task 6;
            // for this intermediate commit it returns a clear "not yet
            // implemented" error rather than silently calling into the
            // legacy single-ID path with the wrong arguments.
            if !bug_ids.is_empty() || ids.len() != 1 {
                return Err(crate::error::BzrError::Other(
                    "bulk attachment download dispatch not yet wired (lands in Task 6)".into(),
                ));
            }
            download_single_legacy(&client, ids[0], out.as_deref(), format).await?;
        }
        AttachmentAction::Upload {
            bug_id,
            file,
            summary,
            content_type,
            private,
            is_patch,
            comment,
            comment_private,
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
            if *comment_private {
                flip_new_comment_private(&client, *bug_id, att_id).await?;
            }
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

fn validate_action(action: &AttachmentAction) -> Result<()> {
    match action {
        AttachmentAction::Upload {
            comment_private: true,
            comment: None,
            ..
        } => Err(crate::error::BzrError::InputValidation(
            "--comment-private requires --comment".into(),
        )),
        AttachmentAction::Download { ids, bug_ids, .. } if ids.is_empty() && bug_ids.is_empty() => {
            Err(crate::error::BzrError::InputValidation(
                "specify at least one attachment ID or --bug <ID>".into(),
            ))
        }
        AttachmentAction::Download {
            ids, out: Some(_), ..
        } if ids.len() != 1 => Err(crate::error::BzrError::InputValidation(
            "--out requires exactly one attachment ID".into(),
        )),
        _ => Ok(()),
    }
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

/// Flip the privacy of the comment that `Bug.add_attachment` just
/// created. Identifies the comment by its `attachment_id` field —
/// Bugzilla sets that to the new attachment ID when the comment was
/// posted alongside the upload.
///
/// On any failure between upload and the privacy flip, prints a stderr
/// warning naming the attachment ID and the underlying error, then
/// propagates the original error so the exit code reflects the failure.
/// The attachment is **not** deleted on partial failure (destructive
/// rollback is worse than a public comment the user can re-target).
async fn flip_new_comment_private(
    client: &BugzillaClient,
    bug_id: u64,
    new_attachment_id: u64,
) -> Result<()> {
    let comments = client
        .get_comments_since(bug_id, None)
        .await
        .inspect_err(|e| warn_partial(new_attachment_id, e))?;
    // `attachment_id` is unique per bug: Bugzilla sets it on exactly one
    // comment when `Bug.add_attachment` includes a `comment` body.
    let Some(comment_id) = comments
        .iter()
        .find(|c| c.attachment_id == Some(new_attachment_id))
        .map(|c| c.id)
    else {
        let err = crate::error::BzrError::DataIntegrity(format!(
            "could not locate the new attachment-bound comment on bug #{bug_id} \
             (no comment with attachment_id={new_attachment_id})",
        ));
        warn_partial(new_attachment_id, &err);
        return Err(err);
    };
    let mut map = HashMap::new();
    map.insert(comment_id, true);
    let params = crate::types::UpdateBugParams {
        comment_is_private: map,
        ..Default::default()
    };
    client
        .update_bug(bug_id, &params)
        .await
        .inspect_err(|e| warn_partial(new_attachment_id, e))
}

fn warn_partial(att_id: u64, err: &crate::error::BzrError) {
    let _ = writeln!(
        std::io::stderr(),
        "warning: attachment #{att_id} uploaded but comment privacy flip failed: {err}",
    );
    let _ = writeln!(
        std::io::stderr(),
        "  the comment was created public; mark it private via the Bugzilla web UI or with elevated credentials",
    );
}

/// Single-attachment download: writes one decoded blob to `out` (if
/// supplied) or to the attachment's stored `file_name` in the current
/// directory. Behavior matches the original inline arm — the function
/// exists to keep `execute()` readable when the bulk path is added.
async fn download_single_legacy(
    client: &BugzillaClient,
    id: u64,
    out: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let (filename, data) = client.download_attachment(id).await?;
    let dest = out.unwrap_or(&filename);
    std::fs::write(dest, &data)?;
    output::print_result(
        &DownloadResult::new(id, dest, data.len()),
        &format!(
            "Downloaded attachment #{id} to {dest} ({} bytes)",
            data.len(),
        ),
        format,
    );
    Ok(())
}

#[cfg(test)]
#[path = "attachment_tests.rs"]
mod tests;
