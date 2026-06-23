use std::path::Path;

use base64::Engine;

use crate::client::BugzillaClient;
use crate::commands::runtime::context::CommandContext;
use crate::error::{io_with_context, Result};
use crate::output::resources::attachment::{
    write_attachment_batch, AttachmentBatchResult, AttachmentDownloadResult, BatchSummary,
    BugDownloadResult, DownloadedFile, TargetStatus,
};
use crate::output::result_types::{write_result, DownloadResult};
use crate::output::writers::Writers;
use crate::types::attachment::Attachment;
use crate::types::common::OutputFormat;

pub(super) struct DownloadArgs<'a> {
    pub(super) ids: &'a [u64],
    pub(super) bug_ids: &'a [u64],
    pub(super) out: Option<&'a str>,
    pub(super) out_dir: &'a str,
}

pub(super) async fn handle(
    args: DownloadArgs<'_>,
    ctx: &CommandContext,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    if args.bug_ids.is_empty() && args.ids.len() == 1 {
        let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
        download_single(&client, args.ids[0], args.out, format, w).await
    } else {
        ensure_batch_out_dir(args.out_dir)?;
        let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
        let targets = BatchTargets {
            ids: args.ids,
            bug_ids: args.bug_ids,
            out_dir: args.out_dir,
        };
        download_batch(&client, targets, format, w).await
    }
}

fn ensure_batch_out_dir(out_dir: &str) -> Result<()> {
    std::fs::create_dir_all(out_dir).map_err(|e| {
        io_with_context(
            format!(
                "failed to create attachment download directory '{}'",
                Path::new(out_dir).display()
            ),
            &e,
        )
    })
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
pub(super) fn safe_basename(name: &str) -> Result<String> {
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
pub(super) fn single_download_dest(
    out: Option<&str>,
    server_filename: &str,
) -> Result<std::path::PathBuf> {
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
pub(super) async fn write_one_attachment(
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
