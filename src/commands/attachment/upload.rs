use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::client::BugzillaClient;
use crate::commands::runtime::input::attachment_input::{
    prepare_attachment_params, AttachmentInput,
};
use crate::commands::runtime::invocation::CommandContext;
use crate::commands::runtime::mutation::ensure_batch_complete;
use crate::error::Result;
use crate::output::result_types::{
    write_result, BatchUploadResult, UploadFailure, UploadResult, UploadTarget,
};
use crate::output::writers::Writers;
use crate::types::attachment::UploadAttachmentParams;
use crate::types::bug::UpdateBugParams;
use crate::types::output::OutputFormat;

pub(super) async fn handle(
    args: &crate::cli::UploadArgs,
    ctx: &CommandContext,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let mut prepared = prepare_upload(args)?;
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    if let [bug_id] = args.bug_ids[..] {
        return upload_single(&client, &mut prepared, bug_id, format, w).await;
    }
    upload_batch(&client, &mut prepared, &args.bug_ids, format, w).await
}

/// One bug: the original shape, byte for byte. Kept separate from the
/// batch path so the common case's result contract cannot drift.
///
/// `prepared` is borrowed mutably and re-targeted in place rather than
/// cloned: `UploadAttachmentParams.data` is the whole file as `Vec<u8>`
/// (base64 is applied by `serialize_data_as_base64` at request time), so a
/// clone per bug would hold the file body once per target for no reason.
async fn upload_single(
    client: &BugzillaClient,
    prepared: &mut PreparedUpload,
    bug_id: u64,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    prepared.params.bug_id = bug_id;
    let att_id = client.upload_attachment(&prepared.params).await?;
    if prepared.comment_private {
        flip_new_comment_private(client, bug_id, att_id, w).await?;
    }
    write_result(
        &UploadResult::new(att_id, bug_id, prepared.size),
        &format!(
            "Uploaded attachment #{att_id} to bug #{bug_id} ({} bytes)",
            prepared.size,
        ),
        format,
        w.out,
    );
    Ok(())
}

/// Two or more bugs: upload the same prepared payload to each, recording
/// per-bug outcomes and continuing past a failure. A bug whose upload
/// succeeded but whose `--comment-private` flip failed appears in both
/// `uploaded` and `failed`.
async fn upload_batch(
    client: &BugzillaClient,
    prepared: &mut PreparedUpload,
    bug_ids: &[u64],
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let mut uploaded = Vec::new();
    let mut failed = Vec::new();
    for &bug_id in bug_ids {
        prepared.params.bug_id = bug_id;
        match client.upload_attachment(&prepared.params).await {
            Ok(att_id) => {
                uploaded.push(UploadTarget {
                    bug_id,
                    attachment_id: att_id,
                });
                if prepared.comment_private {
                    // `flip_new_comment_private` narrates its own failure to
                    // stderr for the single-bug path. On the batch path the
                    // result body and the table renderer already report it,
                    // so the quiet variant is used and one failure produces
                    // one message.
                    if let Err(e) = flip_new_comment_private_quiet(client, bug_id, att_id).await {
                        failed.push(UploadFailure::comment_private(bug_id, e.to_string()));
                    }
                }
            }
            Err(e) => failed.push(UploadFailure::new(bug_id, e.to_string())),
        }
    }
    let result = BatchUploadResult::new(prepared.size, uploaded, failed);
    write_batch_upload(&result, format, w);
    // Counted over distinct bug IDs (each drawn from `bug_ids`) rather than
    // subtracted from `result.failed.len()`, so `failed_targets <= bug_ids.len()`
    // is a property of the set rather than an invariant hand-maintained across
    // the loop's two `push` sites (upload failure, comment-private failure).
    let failed_bugs: HashSet<u64> = result.failed.iter().map(|f| f.bug_id).collect();
    let failed_targets = failed_bugs.len();
    ensure_batch_complete(bug_ids.len() - failed_targets, failed_targets)
}

/// Render a [`BatchUploadResult`]. JSON and NDJSON emit the object; the
/// table form prints one line per upload and sends each failure to stderr.
/// A `comment_private` entry is **not** rendered as an upload failure: the
/// attachment exists, and telling the operator the upload failed invites a
/// retry that leaves a second attachment Bugzilla cannot delete.
fn write_batch_upload(result: &BatchUploadResult, format: OutputFormat, w: &mut Writers<'_>) {
    match format {
        OutputFormat::Json | OutputFormat::Ndjson => write_result(result, "", format, w.out),
        OutputFormat::Table => {
            for t in &result.uploaded {
                let _ = writeln!(
                    w.out,
                    "Uploaded attachment #{} to bug #{} ({} bytes)",
                    t.attachment_id, t.bug_id, result.size,
                );
            }
            for f in &result.failed {
                if f.step.is_some() {
                    let _ = writeln!(
                        w.err,
                        "Uploaded to bug #{} but could not make the comment private: {}",
                        f.bug_id, f.error,
                    );
                } else {
                    let _ = writeln!(w.err, "Failed to upload to bug #{}: {}", f.bug_id, f.error);
                }
            }
        }
    }
}

struct PreparedUpload {
    params: UploadAttachmentParams,
    size: usize,
    comment_private: bool,
}

fn prepare_upload(args: &crate::cli::UploadArgs) -> Result<PreparedUpload> {
    let crate::cli::UploadArgs {
        bug_ids: _,
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
    let is_private = super::resolve_bool_flag(*private, *no_private).unwrap_or(false);
    let is_patch = super::resolve_bool_flag(*patch, *no_patch).unwrap_or(false);
    let flags = crate::commands::runtime::input::flags::parse_flags(flag)?;
    let comment = resolve_upload_comment(
        comment.as_deref(),
        comment_file.as_deref(),
        *comment_private,
    )?;
    let (params, size) = prepare_attachment_params(AttachmentInput {
        file: Path::new(file),
        summary: summary.as_deref(),
        content_type: content_type.as_deref(),
        is_patch,
        is_private,
        comment,
        flags,
    })?;
    Ok(PreparedUpload {
        params,
        size,
        comment_private: *comment_private,
    })
}

fn resolve_upload_comment(
    comment: Option<&str>,
    comment_file: Option<&Path>,
    comment_private: bool,
) -> Result<Option<String>> {
    crate::commands::runtime::shared::materialize_optional_comment_body(
        comment,
        comment_file,
        comment_private,
    )
}

/// Re-export so the existing `super::guess_content_type` tests resolve to the
/// single shared content-type table. Test-only: production callers use
/// `prepare_attachment_params`, which guesses internally.
#[cfg(test)]
pub(super) use crate::commands::runtime::input::attachment_input::guess_content_type;

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
    flip_new_comment_private_quiet(client, bug_id, new_attachment_id)
        .await
        .inspect_err(|e| warn_partial(new_attachment_id, e, w))
}

/// The single-bug workflow's body, without stderr narration. Shared by
/// [`flip_new_comment_private`] (single-bug, which wraps this with
/// `warn_partial`) and the batch path (which records the failure in the
/// result body instead of printing it directly).
async fn flip_new_comment_private_quiet(
    client: &BugzillaClient,
    bug_id: u64,
    new_attachment_id: u64,
) -> Result<()> {
    let comments = client.get_comments_since(bug_id, None).await?;
    // `attachment_id` is unique per bug: Bugzilla sets it on exactly one
    // comment when `Bug.add_attachment` includes a `comment` body.
    let Some(comment_id) = comments
        .iter()
        .find(|c| c.attachment_id == Some(new_attachment_id))
        .map(|c| c.id)
    else {
        return Err(crate::error::BzrError::DataIntegrity(format!(
            "could not locate the new attachment-bound comment on bug #{bug_id} \
             (no comment with attachment_id={new_attachment_id})",
        )));
    };
    let mut map = HashMap::new();
    map.insert(comment_id, true);
    let params = UpdateBugParams {
        comment_is_private: map,
        ..Default::default()
    };
    client.update_bug(bug_id, &params).await
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

#[cfg(test)]
#[path = "upload_tests.rs"]
mod tests;
