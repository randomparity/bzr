//! Field-combination validation for `bug update`, run before the API payload
//! is built so invalid flag combinations fail fast.

use crate::cli::UpdateArgs;
use crate::error::Result;

use super::BugUpdateDraft;

/// Reject invalid field combinations before building the API payload.
pub(super) fn validate_draft(draft: &BugUpdateDraft, ids: &[u64]) -> Result<()> {
    if draft.dupe_of.is_some() && (draft.status.is_some() || draft.resolution.is_some()) {
        return Err(crate::error::BzrError::input(
            "--dupe-of cannot be combined with --status or --resolution".into(),
        ));
    }
    if draft.alias.is_some() && ids.len() > 1 {
        return Err(crate::error::BzrError::input(
            "--alias can only be used when updating one bug".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_args(args: &UpdateArgs) -> Result<()> {
    let dash = |value: Option<&str>| value == Some("-");
    crate::commands::runtime::input::extra_fields::reject_stdin_conflict(
        args.field_json.as_deref(),
        &[
            ("--from-json -", dash(args.from_json.as_deref())),
            ("--comment -", dash(args.comment.as_deref())),
            (
                "--comment-file -",
                args.comment_file.as_deref() == Some(std::path::Path::new("-")),
            ),
        ],
    )?;
    validate_draft(&BugUpdateDraft::from_cli(args), &args.ids)
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
