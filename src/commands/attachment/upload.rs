use std::collections::HashMap;
use std::path::Path;

use crate::client::BugzillaClient;
use crate::commands::runtime::context::CommandContext;
use crate::error::{io_with_context, Result};
use crate::output::result_types::{write_result, UploadResult};
use crate::output::writers::Writers;
use crate::types::attachment::UploadAttachmentParams;
use crate::types::bug::UpdateBugParams;
use crate::types::common::OutputFormat;

pub(super) async fn handle(
    args: &crate::cli::UploadArgs,
    ctx: &CommandContext,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let prepared = prepare_upload(args)?;
    let bug_id = prepared.params.bug_id;
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let att_id = client.upload_attachment(&prepared.params).await?;
    if prepared.comment_private {
        flip_new_comment_private(&client, bug_id, att_id, w).await?;
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

struct PreparedUpload {
    params: UploadAttachmentParams,
    size: usize,
    comment_private: bool,
}

fn prepare_upload(args: &crate::cli::UploadArgs) -> Result<PreparedUpload> {
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
    let is_private = super::resolve_bool_flag(*private, *no_private).unwrap_or(false);
    let is_patch = super::resolve_bool_flag(*patch, *no_patch).unwrap_or(false);
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
    let flags = crate::commands::runtime::flags::parse_flags(flag)?;
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
    Ok(PreparedUpload {
        params: upload_params,
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

pub(super) fn guess_content_type(filename: &str) -> &'static str {
    let Some(ext) = Path::new(filename).extension().and_then(|ext| ext.to_str()) else {
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
    let params = UpdateBugParams {
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
