use std::io::{self, Write as _};

use colored::Colorize;
use serde::Serialize;

use super::formatting::{print_field, print_formatted, print_optional_field};
use crate::types::{Attachment, OutputFormat};

pub fn print_attachments(attachments: &[Attachment], format: OutputFormat) {
    print_formatted(attachments, format, |attachments| {
        if attachments.is_empty() {
            let _ = writeln!(io::stdout(), "No attachments.");
            return;
        }
        for a in attachments {
            let patch = if a.is_patch { " [PATCH]" } else { "" };
            let obsolete = if a.is_obsolete { " [OBSOLETE]" } else { "" };
            let private = if a.is_private { " [PRIVATE]" } else { "" };
            let _ = writeln!(
                io::stdout(),
                "{} #{} - {}{}{}{}",
                "Attachment".bold(),
                a.id,
                a.summary.bold(),
                patch.cyan(),
                obsolete.red(),
                private.red(),
            );
            print_field(
                "File",
                &format!("{} ({}, {} bytes)", a.file_name, a.content_type, a.size),
            );
            print_optional_field("Creator", a.creator.as_deref());
            print_optional_field("Created", a.creation_time.as_deref());
            let _ = writeln!(io::stdout());
        }
    });
}

/// Top-level payload for `bzr attachment download` in bulk mode.
///
/// Single-ID legacy mode continues to use [`super::DownloadResult`].
#[derive(Debug, Serialize)]
#[non_exhaustive]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by print_attachment_batch renderer (Task 4) and dispatch (Task 6)"
    )
)]
pub struct AttachmentBatchResult {
    pub out_dir: String,
    pub bug_results: Vec<BugDownloadResult>,
    pub attachment_results: Vec<AttachmentDownloadResult>,
    pub summary: BatchSummary,
}

/// Result of `--bug <ID>` for a single bug. Carries every file the
/// bulk path successfully wrote to disk for this bug, and (if any
/// attachment failed) the first error message encountered.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct BugDownloadResult {
    pub bug_id: u64,
    pub status: TargetStatus,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<DownloadedFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Result of a positional attachment-ID argument in bulk mode. The
/// `bug_id` field is populated from the API response when reachable,
/// even on per-attachment failure (so the user can correlate the
/// failure to the bug it belongs to).
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct AttachmentDownloadResult {
    pub attachment_id: u64,
    pub status: TargetStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bug_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One on-disk artifact produced by the bulk path.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct DownloadedFile {
    pub attachment_id: u64,
    pub path: String,
    pub bytes: usize,
}

/// Aggregate counters for the batch run.
///
/// `succeeded` counts attachments written to disk; `failed` counts the
/// sum of bug-level errors and per-attachment failures. A bug with
/// three attachments where two are written and one fails contributes
/// 2 to `succeeded` and 1 to `failed`.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct BatchSummary {
    pub succeeded: usize,
    pub failed: usize,
    pub total_bytes: usize,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[non_exhaustive]
#[serde(rename_all = "lowercase")]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by print_attachment_batch renderer (Task 4) and dispatch (Task 6)"
    )
)]
pub enum TargetStatus {
    Ok,
    Error,
}

#[cfg(test)]
#[path = "attachment_tests.rs"]
mod tests;
