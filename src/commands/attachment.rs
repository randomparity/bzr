use std::collections::HashMap;
use std::path::Path;

use base64::Engine;

use crate::cli::AttachmentAction;
use crate::client::BugzillaClient;
use crate::commands::runtime::context::CommandContext;
use crate::error::{io_with_context, Result};
use crate::output::resources::attachment::{
    write_attachment, write_attachment_batch, write_attachments, AttachmentBatchResult,
    AttachmentDownloadResult, BatchSummary, BugDownloadResult, DownloadedFile, TargetStatus,
};
use crate::output::result_types::{
    write_result, ActionResult, DownloadResult, ResourceKind, UploadResult,
};
use crate::output::writers::Writers;
use crate::types::Attachment;
use crate::types::OutputFormat;
use crate::types::{UpdateAttachmentParams, UploadAttachmentParams};

pub(crate) fn requires_credentials(action: &AttachmentAction) -> Option<&'static str> {
    match action {
        AttachmentAction::List { .. }
        | AttachmentAction::View { .. }
        | AttachmentAction::Download { .. } => None,
        AttachmentAction::Upload(_) => Some("attachment upload"),
        AttachmentAction::Update(_) => Some("attachment update"),
    }
}

/// Collapse a `--flag` / `--no-flag` presence pair into a tri-state.
///
/// Returns `Some(true)` for the positive flag, `Some(false)` for the negative,
/// and `None` when neither is given (caller decides the unset meaning). clap's
/// `overrides_with` guarantees the two are never both `true`, so the order of
/// the checks is irrelevant.
fn resolve_bool_flag(yes: bool, no: bool) -> Option<bool> {
    if yes {
        Some(true)
    } else if no {
        Some(false)
    } else {
        None
    }
}

pub async fn execute(
    action: &AttachmentAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    validate_action(action)?;
    let format = ctx.format();
    let client = super::runtime::shared::connect_and_configure(ctx).await?;

    match action {
        AttachmentAction::List { bug_id } => list_attachments(&client, *bug_id, format, w).await?,
        AttachmentAction::View { attachment_id } => {
            view_attachment(&client, *attachment_id, format, w).await?;
        }
        AttachmentAction::Download {
            ids,
            bug_ids,
            out,
            out_dir,
        } => {
            if bug_ids.is_empty() && ids.len() == 1 {
                download_single(&client, ids[0], out.as_deref(), format, w).await?;
            } else {
                let targets = BatchTargets {
                    ids,
                    bug_ids,
                    out_dir,
                };
                download_batch(&client, targets, format, w).await?;
            }
        }
        AttachmentAction::Upload(args) => upload(&client, args, format, w).await?,
        AttachmentAction::Update(args) => update(&client, args, format, w).await?,
    }
    Ok(())
}

async fn upload(
    client: &BugzillaClient,
    args: &crate::cli::UploadArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let crate::cli::UploadArgs {
        bug_id,
        file,
        summary,
        content_type,
        private,
        no_private,
        patch,
        no_patch,
        comment,
        comment_file,
        comment_private,
        flag,
    } = args;
    // Upload has no "leave unchanged" state: absent both flags is public /
    // non-patch.
    let is_private = resolve_bool_flag(*private, *no_private).unwrap_or(false);
    let is_patch = resolve_bool_flag(*patch, *no_patch).unwrap_or(false);
    let path = Path::new(file);
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or(file);
    let data = std::fs::read(path).map_err(|e| {
        io_with_context(
            format!("failed to read attachment upload file '{}'", path.display()),
            &e,
        )
    })?;
    let summary = summary.as_deref().unwrap_or(file_name);
    let ct = match (content_type.as_deref(), is_patch) {
        (Some(explicit), _) => explicit.to_string(),
        (None, true) => "text/plain".to_string(),
        (None, false) => guess_content_type(file_name).to_string(),
    };
    let flags = super::runtime::flags::parse_flags(flag)?;
    let comment = resolve_upload_comment(
        comment.as_deref(),
        comment_file.as_deref(),
        *comment_private,
    )?;
    let size = data.len();
    let upload_params = UploadAttachmentParams {
        bug_id: *bug_id,
        file_name: file_name.to_string(),
        summary: summary.to_string(),
        content_type: ct,
        data,
        flags,
        is_private,
        comment,
        is_patch,
    };
    let att_id = client.upload_attachment(&upload_params).await?;
    if *comment_private {
        flip_new_comment_private(client, *bug_id, att_id, w).await?;
    }
    write_result(
        &UploadResult::new(att_id, *bug_id, size),
        &format!("Uploaded attachment #{att_id} to bug #{bug_id} ({size} bytes)"),
        format,
        w.out,
    );
    Ok(())
}

fn resolve_upload_comment(
    comment: Option<&str>,
    comment_file: Option<&Path>,
    comment_private: bool,
) -> Result<Option<String>> {
    let required_body_message =
        comment_private.then_some("--comment-private requires --comment or --comment-file");
    super::runtime::shared::materialize_non_empty_comment_body(
        super::runtime::shared::classify_body_source(
            comment,
            comment_file,
            "--comment",
            "--comment-file",
        )?,
        "--comment-file",
        required_body_message,
    )
}

async fn update(
    client: &BugzillaClient,
    args: &crate::cli::AttachmentUpdateArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let crate::cli::AttachmentUpdateArgs {
        id,
        summary,
        file_name,
        content_type,
        obsolete,
        no_obsolete,
        patch,
        no_patch,
        private,
        no_private,
        flag,
    } = args;
    let flags = super::runtime::flags::parse_flags(flag)?;
    let params = UpdateAttachmentParams {
        summary: summary.clone(),
        file_name: file_name.clone(),
        content_type: content_type.clone(),
        // None = leave unchanged; Some(b) = set explicitly.
        is_obsolete: resolve_bool_flag(*obsolete, *no_obsolete),
        is_patch: resolve_bool_flag(*patch, *no_patch),
        is_private: resolve_bool_flag(*private, *no_private),
        flags,
    };
    client.update_attachment(*id, &params).await?;
    write_result(
        &ActionResult::updated(*id, ResourceKind::Attachment),
        &format!("Updated attachment #{id}"),
        format,
        w.out,
    );
    Ok(())
}

async fn list_attachments(
    client: &BugzillaClient,
    bug_id: u64,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let attachments = client.get_attachments(bug_id).await?;
    write_attachments(&attachments, format, w.out);
    Ok(())
}

async fn view_attachment(
    client: &BugzillaClient,
    attachment_id: u64,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let attachment = client.get_attachment_metadata(attachment_id).await?;
    write_attachment(&attachment, format, w.out);
    Ok(())
}

fn validate_action(action: &AttachmentAction) -> Result<()> {
    match action {
        AttachmentAction::Upload(crate::cli::UploadArgs {
            comment_private: true,
            comment: None,
            comment_file: None,
            ..
        }) => Err(crate::error::BzrError::InputValidation(
            "--comment-private requires --comment or --comment-file".into(),
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
        AttachmentAction::Update(args) if !update_has_changes(args) => {
            Err(crate::error::BzrError::InputValidation(
                "no attachment fields to update; specify at least one of --summary, --file-name, \
                 --content-type, --obsolete/--no-obsolete, --patch/--no-patch, \
                 --private/--no-private, or --flag"
                    .into(),
            ))
        }
        _ => Ok(()),
    }
}

fn update_has_changes(args: &crate::cli::AttachmentUpdateArgs) -> bool {
    args.summary.is_some()
        || args.file_name.is_some()
        || args.content_type.is_some()
        || args.obsolete
        || args.no_obsolete
        || args.patch
        || args.no_patch
        || args.private
        || args.no_private
        || !args.flag.is_empty()
}

/// Maps file extensions (compared case-insensitively) to their MIME type.
const CONTENT_TYPES: &[(&[&str], &str)] = &[
    (
        &[
            "txt", "log", "c", "h", "cpp", "rs", "py", "sh", "pl", "rb", "js", "ts",
        ],
        "text/plain",
    ),
    (&["html", "htm"], "text/html"),
    (&["json"], "application/json"),
    (&["xml"], "application/xml"),
    (&["pdf"], "application/pdf"),
    (&["png"], "image/png"),
    (&["jpg", "jpeg"], "image/jpeg"),
    (&["gif"], "image/gif"),
    (&["svg"], "image/svg+xml"),
    (&["gz", "tgz"], "application/gzip"),
    (&["zip"], "application/zip"),
    (&["tar"], "application/x-tar"),
    (&["patch", "diff"], "text/x-diff"),
];

fn guess_content_type(filename: &str) -> &'static str {
    let Some(ext) = filename.rsplit('.').next() else {
        return "application/octet-stream";
    };
    for (extensions, content_type) in CONTENT_TYPES {
        if extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)) {
            return content_type;
        }
    }
    "application/octet-stream"
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
    w: &mut Writers<'_>,
) -> Result<()> {
    let comments = client
        .get_comments_since(bug_id, None)
        .await
        .inspect_err(|e| warn_partial(new_attachment_id, e, w))?;
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
        warn_partial(new_attachment_id, &err, w);
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
        .inspect_err(|e| warn_partial(new_attachment_id, e, w))
}

fn warn_partial(att_id: u64, err: &crate::error::BzrError, w: &mut Writers<'_>) {
    let _ = writeln!(
        w.err,
        "warning: attachment #{att_id} uploaded but comment privacy flip failed: {err}",
    );
    let _ = writeln!(
        w.err,
        "  the comment was created public; mark it private via the Bugzilla web UI or with elevated credentials",
    );
}

fn ensure_batch_complete(succeeded: usize, failed: usize) -> Result<()> {
    if failed > 0 {
        Err(crate::error::BzrError::BatchPartialFailure { succeeded, failed })
    } else {
        Ok(())
    }
}

/// Reduce a server-supplied attachment file name to its final path
/// component, rejecting names that carry no usable basename (`""`,
/// `"."`, `".."`, `"foo/.."`). Bugzilla returns `file_name` verbatim from
/// whoever uploaded the attachment, so it must never be trusted as a
/// write path — `../../etc/foo` or `/etc/foo` would otherwise escape the
/// target directory.
fn safe_basename(name: &str) -> Result<String> {
    match Path::new(name).file_name().and_then(|n| n.to_str()) {
        Some(base) if base != "." && base != ".." && !base.is_empty() => Ok(base.to_string()),
        _ => Err(crate::error::BzrError::InputValidation(format!(
            "attachment file name {name:?} has no usable file component",
        ))),
    }
}

/// Resolve the destination for a single-attachment download. An explicit
/// `--out` is the user's own choice and is honored verbatim; otherwise the
/// untrusted server file name is reduced to a safe basename in the current
/// directory.
fn single_download_dest(out: Option<&str>, server_filename: &str) -> Result<std::path::PathBuf> {
    match out {
        Some(path) => Ok(std::path::PathBuf::from(path)),
        None => Ok(std::path::PathBuf::from(safe_basename(server_filename)?)),
    }
}

/// Single-attachment download: writes one decoded blob to stdout when
/// `--out -` is supplied, otherwise to `out` or the attachment's stored
/// `file_name` in the current directory. Paired with `download_batch`
/// for bulk shapes; both paths are first-class.
async fn download_single(
    client: &BugzillaClient,
    id: u64,
    out: Option<&str>,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let (filename, data) = client.download_attachment(id).await?;
    if out == Some("-") {
        w.out.write_all(&data).map_err(|e| {
            io_with_context(format!("failed to write attachment #{id} to stdout"), &e)
        })?;
        return Ok(());
    }
    let dest = single_download_dest(out, &filename)?;
    std::fs::write(&dest, &data).map_err(|e| {
        io_with_context(
            format!("failed to write attachment #{id} to '{}'", dest.display()),
            &e,
        )
    })?;
    let dest = dest.to_string_lossy().into_owned();
    write_result(
        &DownloadResult::new(id, dest.as_str(), data.len()),
        &format!(
            "Downloaded attachment #{id} to {dest} ({} bytes)",
            data.len(),
        ),
        format,
        w.out,
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
    std::fs::create_dir_all(&bug_subdir).map_err(|e| {
        io_with_context(
            format!(
                "failed to create attachment download directory '{}'",
                bug_subdir.display()
            ),
            &e,
        )
    })?;
    let dest = bug_subdir.join(format!("{}.{}", att.id, safe_basename(&att.file_name)?));
    let dest_str = dest.to_string_lossy().into_owned();
    std::fs::write(&dest, &bytes).map_err(|e| {
        io_with_context(
            format!(
                "failed to write attachment #{} to '{}'",
                att.id,
                dest.display()
            ),
            &e,
        )
    })?;

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

/// Targets for the multi-source `Download` batch path. `ids` are
/// individual attachment IDs; `bug_ids` resolve to every attachment on
/// the named bugs. Both lists may be non-empty (the two streams merge
/// into one `AttachmentBatchResult`). `out_dir` is the shared
/// destination directory.
#[derive(Clone, Copy)]
struct BatchTargets<'a> {
    ids: &'a [u64],
    bug_ids: &'a [u64],
    out_dir: &'a str,
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
    targets: BatchTargets<'_>,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    std::fs::create_dir_all(targets.out_dir).map_err(|e| {
        io_with_context(
            format!(
                "failed to create attachment download directory '{}'",
                Path::new(targets.out_dir).display()
            ),
            &e,
        )
    })?;

    let mut bug_results: Vec<BugDownloadResult> = Vec::new();
    let mut attachment_results: Vec<AttachmentDownloadResult> = Vec::new();

    for &bug_id in targets.bug_ids {
        bug_results.push(download_bug_target(client, bug_id, targets.out_dir).await);
    }

    for &att_id in targets.ids {
        attachment_results.push(download_attachment_target(client, att_id, targets.out_dir).await);
    }

    let summary = BatchSummary::from_results(&bug_results, &attachment_results);
    let result = AttachmentBatchResult {
        out_dir: targets.out_dir.to_string(),
        bug_results,
        attachment_results,
        summary,
    };

    write_attachment_batch(&result, format, w.out, w.err);
    ensure_batch_complete(result.summary.succeeded, result.summary.failed)
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
