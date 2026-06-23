//! Output and result formatting for `bug update`: human-readable table lines,
//! the JSON/NDJSON batch envelope, and the dry-run preview.

use crate::output::result_types::{write_result, BatchResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::bug::UpdateBugParams;
use crate::types::common::OutputFormat;

const COMMENT_SUFFIX: &str = " (with comment)";

pub(super) fn comment_suffix(present: bool) -> &'static str {
    if present {
        COMMENT_SUFFIX
    } else {
        ""
    }
}

pub(crate) fn write_batch_result(
    batch: &BatchResult,
    format: OutputFormat,
    with_comment: bool,
    w: &mut Writers<'_>,
) {
    match format {
        OutputFormat::Json | OutputFormat::Ndjson => {
            write_result(batch, "", format, w.out);
        }
        OutputFormat::Table => {
            if !batch.succeeded.is_empty() {
                let ids_str: Vec<String> =
                    batch.succeeded.iter().map(|id| format!("#{id}")).collect();
                let suffix = comment_suffix(with_comment);
                let _ = writeln!(w.out, "Updated bugs: {}{suffix}", ids_str.join(", "));
            }
            for f in &batch.failed {
                let _ = writeln!(w.err, "Failed to update bug #{}: {}", f.id, f.error);
            }
        }
    }
}

/// Emit the would-be update without writing: the affected IDs and the payload
/// that would be sent, marked `"action":"dry-run"`. Shared by `bug update` and
/// the convenience verbs.
pub(super) fn write_update_dry_run(
    ids: &[u64],
    params: &UpdateBugParams,
    format: OutputFormat,
    w: &mut Writers<'_>,
) {
    let ids_str: Vec<String> = ids.iter().map(|id| format!("#{id}")).collect();
    let suffix = comment_suffix(params.comment.is_some());
    write_result(
        &DryRunResult::new(ResourceKind::Bug, ids, params),
        &format!(
            "Dry run: would update bug(s) {}{suffix} (no changes made)",
            ids_str.join(", ")
        ),
        format,
        w.out,
    );
}
