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
/// Single-ID mode continues to use [`super::DownloadResult`].
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct AttachmentBatchResult {
    pub out_dir: String,
    pub bug_results: Vec<BugDownloadResult>,
    pub attachment_results: Vec<AttachmentDownloadResult>,
    pub summary: BatchSummary,
}

/// Result of `--bug <ID>` for a single bug. Carries every file the
/// bulk path successfully wrote to disk for this bug. If any
/// attachment failed, `error` holds the first error message
/// encountered; subsequent failures within the same bug are not
/// retained — the renderer surfaces only one error per bug.
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
/// `succeeded` counts attachments successfully written to disk
/// (across both `--bug` and positional-ID targets). `failed` counts
/// failed *targets* — each `--bug <ID>` whose result is `Error` plus
/// each positional attachment-ID whose result is `Error`. A bug whose
/// listing succeeds but where some internal writes fail contributes
/// each successful write to `succeeded` and one to `failed` for the
/// bug itself.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct BatchSummary {
    pub succeeded: usize,
    pub failed: usize,
    pub total_bytes: usize,
}

impl BatchSummary {
    /// Compute the summary post-hoc from the populated result lists.
    pub fn from_results(
        bug_results: &[BugDownloadResult],
        attachment_results: &[AttachmentDownloadResult],
    ) -> Self {
        let succeeded = bug_results.iter().map(|b| b.files.len()).sum::<usize>()
            + attachment_results
                .iter()
                .filter(|a| a.status == TargetStatus::Ok)
                .count();
        let failed = bug_results
            .iter()
            .filter(|b| b.status == TargetStatus::Error)
            .count()
            + attachment_results
                .iter()
                .filter(|a| a.status == TargetStatus::Error)
                .count();
        let total_bytes = bug_results
            .iter()
            .flat_map(|b| &b.files)
            .map(|f| f.bytes)
            .sum::<usize>()
            + attachment_results
                .iter()
                .filter_map(|a| a.bytes)
                .sum::<usize>();
        Self {
            succeeded,
            failed,
            total_bytes,
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[non_exhaustive]
#[serde(rename_all = "lowercase")]
pub enum TargetStatus {
    Ok,
    Error,
}

/// Render an [`AttachmentBatchResult`] in the requested format.
///
/// Successful entries print to stdout; bug-level failures and
/// per-attachment failures print to stderr (one line each), so
/// `2>/dev/null` quiets warnings without losing the success listing.
/// JSON mode emits a single object on stdout — no stderr writes.
pub fn print_attachment_batch(result: &AttachmentBatchResult, format: OutputFormat) {
    print_formatted(result, format, |r| {
        for bug in &r.bug_results {
            match bug.status {
                TargetStatus::Ok => {
                    let _ = writeln!(
                        io::stdout(),
                        "{} {}: {} files saved",
                        "Bug".bold(),
                        format!("#{}", bug.bug_id).bold(),
                        bug.files.len(),
                    );
                    for file in &bug.files {
                        let _ = writeln!(io::stdout(), "  → {} ({} bytes)", file.path, file.bytes);
                    }
                }
                TargetStatus::Error => {
                    let _ = writeln!(
                        io::stderr(),
                        "Bug #{}: {}",
                        bug.bug_id,
                        bug.error.as_deref().unwrap_or("error"),
                    );
                    for file in &bug.files {
                        let _ = writeln!(
                            io::stdout(),
                            "  → {} ({} bytes) [partial]",
                            file.path,
                            file.bytes,
                        );
                    }
                }
            }
        }
        for att in &r.attachment_results {
            match att.status {
                TargetStatus::Ok => {
                    let _ = writeln!(
                        io::stdout(),
                        "{} #{}: {} ({} bytes)",
                        "Attachment".bold(),
                        att.attachment_id,
                        att.path.as_deref().unwrap_or("?"),
                        att.bytes.unwrap_or(0),
                    );
                }
                TargetStatus::Error => {
                    let _ = writeln!(
                        io::stderr(),
                        "Attachment #{}: {}",
                        att.attachment_id,
                        att.error.as_deref().unwrap_or("error"),
                    );
                }
            }
        }
        let _ = writeln!(
            io::stdout(),
            "{} {} succeeded, {} failed, {} total bytes",
            "Summary:".bold(),
            r.summary.succeeded,
            r.summary.failed,
            r.summary.total_bytes,
        );
    });
}

#[cfg(test)]
#[path = "attachment_tests.rs"]
mod tests;
