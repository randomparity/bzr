//! Shared attachment-preparation: read a file from disk and build the
//! [`UploadAttachmentParams`] for a `Bug.add_attachment` POST.
//!
//! Both `attachment upload` and the compound `bug create` path need the same
//! "read file → resolve summary → resolve content type → base64 params" logic;
//! it lives here so the two callers stay byte-for-byte consistent. The
//! content-type guess table is owned here too.

use std::path::Path;

use crate::error::{io_with_context, Result};
use crate::types::attachment::UploadAttachmentParams;
use crate::types::flag::FlagUpdate;

/// Everything needed to prepare one attachment upload. Borrows where it can so
/// callers pass slices of already-parsed CLI/JSON input.
pub(crate) struct AttachmentInput<'a> {
    pub file: &'a Path,
    /// Attachment summary; an absent or whitespace-only value falls back to the
    /// filename.
    pub summary: Option<&'a str>,
    /// MIME type override; absent uses the patch default or the extension guess.
    pub content_type: Option<&'a str>,
    pub is_patch: bool,
    pub is_private: bool,
    /// Optional comment posted alongside the attachment.
    pub comment: Option<String>,
    pub flags: Vec<FlagUpdate>,
}

/// Read `input.file` and build the upload params plus the file's byte size.
///
/// The summary defaults to the filename when not given (or given empty); the
/// content type resolves explicit → patch (`text/plain`) → extension guess. A
/// file that cannot be read is an I/O error naming the path.
pub(crate) fn prepare_attachment_params(
    input: AttachmentInput,
) -> Result<(UploadAttachmentParams, usize)> {
    let file_name = input
        .file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_else(|| input.file.to_str().unwrap_or("attachment"))
        .to_string();
    let data = std::fs::read(input.file).map_err(|e| {
        io_with_context(
            format!(
                "failed to read attachment upload file '{}'",
                input.file.display()
            ),
            &e,
        )
    })?;
    let summary = match input.summary {
        Some(s) if !s.trim().is_empty() => s.to_string(),
        _ => file_name.clone(),
    };
    let content_type = match (input.content_type, input.is_patch) {
        (Some(explicit), _) if !explicit.trim().is_empty() => explicit.to_string(),
        (_, true) => "text/plain".to_string(),
        (_, false) => guess_content_type(&file_name).to_string(),
    };
    let size = data.len();
    let params = UploadAttachmentParams {
        bug_id: 0,
        file_name,
        summary,
        content_type,
        data,
        flags: input.flags,
        is_private: input.is_private,
        comment: input.comment,
        is_patch: input.is_patch,
    };
    Ok((params, size))
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

/// Guess a MIME type from a filename extension, defaulting to
/// `application/octet-stream`.
pub(crate) fn guess_content_type(filename: &str) -> &'static str {
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

#[cfg(test)]
#[path = "attachment_input_tests.rs"]
mod tests;
