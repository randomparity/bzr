use std::collections::HashMap;
use std::io::Write as _;
use std::path::Path;

use base64::Engine;

use crate::cli::AttachmentAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::{
    self, ActionResult, AttachmentBatchResult, AttachmentDownloadResult, BatchSummary,
    BugDownloadResult, DownloadResult, DownloadedFile, ResourceKind, TargetStatus, UploadResult,
};
use crate::types::ApiMode;
use crate::types::Attachment;
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
            out_dir,
        } => {
            if bug_ids.is_empty() && ids.len() == 1 {
                // Legacy single shape — validation already ensured `--out`
                // is not paired with `--bug` or with multiple IDs.
                download_single(&client, ids[0], out.as_deref(), format).await?;
            } else {
                download_batch(&client, ids, bug_ids, out_dir, format).await?;
            }
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
/// directory. Paired with `download_batch` for bulk shapes; both paths
/// are first-class.
async fn download_single(
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

/// Decode (or re-fetch) one attachment's bytes and write them to
/// `<out_dir>/<bug_id>/<att_id>.<file_name>`. Surfaces any failure
/// back to the caller as `BzrError`; the caller decides whether to
/// abort or record-and-continue.
async fn write_one_attachment(
    client: &BugzillaClient,
    att: &Attachment,
    out_dir: &str,
) -> Result<DownloadedFile> {
    let bytes = if let Some(b64) = att.data.as_deref() {
        base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| {
                crate::error::BzrError::DataIntegrity(format!(
                    "failed to decode attachment #{}: {e}",
                    att.id,
                ))
            })?
    } else {
        let (_, fetched) = client.download_attachment(att.id).await?;
        fetched
    };

    let bug_subdir = Path::new(out_dir).join(att.bug_id.to_string());
    std::fs::create_dir_all(&bug_subdir)?;
    let dest = bug_subdir.join(format!("{}.{}", att.id, att.file_name));
    let dest_str = dest.to_string_lossy().into_owned();
    std::fs::write(&dest, &bytes)?;

    tracing::info!(
        att_id = att.id,
        bug_id = att.bug_id,
        path = %dest_str,
        bytes = bytes.len(),
        "downloaded attachment",
    );

    Ok(DownloadedFile {
        attachment_id: att.id,
        path: dest_str,
        bytes: bytes.len(),
    })
}

/// Bulk attachment download: walks every `--bug <ID>` then every
/// positional attachment ID, recording successes and per-target
/// failures. On any failure, returns `BatchPartialFailure` (exit 11).
///
/// Pre-flight: creates `out_dir` itself once. If that fails (e.g. an
/// unwritable parent), returns `Io` (exit 6) without entering the
/// loop — better than burying the same error in N per-attachment rows.
async fn download_batch(
    client: &BugzillaClient,
    ids: &[u64],
    bug_ids: &[u64],
    out_dir: &str,
    format: OutputFormat,
) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;

    let mut bug_results: Vec<BugDownloadResult> = Vec::new();
    let mut attachment_results: Vec<AttachmentDownloadResult> = Vec::new();

    for &bug_id in bug_ids {
        bug_results.push(download_bug_target(client, bug_id, out_dir).await);
    }

    for &att_id in ids {
        attachment_results.push(download_attachment_target(client, att_id, out_dir).await);
    }

    let succeeded: usize = bug_results.iter().map(|b| b.files.len()).sum::<usize>()
        + attachment_results
            .iter()
            .filter(|a| a.status == TargetStatus::Ok)
            .count();
    let failed: usize = bug_results
        .iter()
        .filter(|b| b.status == TargetStatus::Error)
        .count()
        + attachment_results
            .iter()
            .filter(|a| a.status == TargetStatus::Error)
            .count();
    let total_bytes: usize = bug_results
        .iter()
        .flat_map(|b| &b.files)
        .map(|f| f.bytes)
        .sum::<usize>()
        + attachment_results
            .iter()
            .filter_map(|a| a.bytes)
            .sum::<usize>();

    let result = AttachmentBatchResult {
        out_dir: out_dir.to_string(),
        bug_results,
        attachment_results,
        summary: BatchSummary {
            succeeded,
            failed,
            total_bytes,
        },
    };

    output::print_attachment_batch(&result, format);

    if failed > 0 {
        return Err(crate::error::BzrError::BatchPartialFailure { succeeded, failed });
    }
    Ok(())
}

/// Download every attachment for one `--bug <ID>` target. Returns a
/// `BugDownloadResult` describing the per-bug outcome — even on a
/// listing-API failure (in which case `files` is empty and `error` is
/// populated).
async fn download_bug_target(
    client: &BugzillaClient,
    bug_id: u64,
    out_dir: &str,
) -> BugDownloadResult {
    let atts = match client.get_attachments(bug_id).await {
        Ok(atts) => atts,
        Err(e) => {
            return BugDownloadResult {
                bug_id,
                status: TargetStatus::Error,
                files: vec![],
                error: Some(e.to_string()),
            };
        }
    };

    let mut files = Vec::new();
    let mut first_error: Option<String> = None;
    for att in &atts {
        match write_one_attachment(client, att, out_dir).await {
            Ok(file) => {
                files.push(file);
            }
            Err(e) => {
                if first_error.is_none() {
                    first_error = Some(e.to_string());
                }
            }
        }
    }
    let status = if first_error.is_some() {
        TargetStatus::Error
    } else {
        TargetStatus::Ok
    };
    BugDownloadResult {
        bug_id,
        status,
        files,
        error: first_error,
    }
}

/// Download one positional attachment-ID target. The returned record
/// retains `bug_id` whenever the metadata fetch succeeds, so users can
/// correlate post-fetch (e.g. write) failures back to a bug.
async fn download_attachment_target(
    client: &BugzillaClient,
    att_id: u64,
    out_dir: &str,
) -> AttachmentDownloadResult {
    let att = match client.get_attachment(att_id).await {
        Ok(att) => att,
        Err(e) => {
            return AttachmentDownloadResult {
                attachment_id: att_id,
                status: TargetStatus::Error,
                bug_id: None,
                path: None,
                bytes: None,
                error: Some(e.to_string()),
            };
        }
    };
    match write_one_attachment(client, &att, out_dir).await {
        Ok(file) => AttachmentDownloadResult {
            attachment_id: att_id,
            status: TargetStatus::Ok,
            bug_id: Some(att.bug_id),
            path: Some(file.path),
            bytes: Some(file.bytes),
            error: None,
        },
        Err(e) => AttachmentDownloadResult {
            attachment_id: att_id,
            status: TargetStatus::Error,
            bug_id: Some(att.bug_id),
            path: None,
            bytes: None,
            error: Some(e.to_string()),
        },
    }
}

#[cfg(test)]
#[path = "attachment_tests.rs"]
mod tests;
