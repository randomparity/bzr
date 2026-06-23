//! Field-combination validation for `bug update`, run before the API payload
//! is built so invalid flag combinations fail fast.

use crate::cli::UpdateArgs;
use crate::error::Result;

use super::BugUpdateDraft;

/// Reject invalid field combinations before building the API payload.
pub(super) fn validate_draft(draft: &BugUpdateDraft, ids: &[u64]) -> Result<()> {
    if draft.dupe_of.is_some() && (draft.status.is_some() || draft.resolution.is_some()) {
        return Err(crate::error::BzrError::InputValidation(
            "--dupe-of cannot be combined with --status or --resolution".into(),
        ));
    }
    if draft.alias.is_some() && ids.len() > 1 {
        return Err(crate::error::BzrError::InputValidation(
            "--alias can only be used when updating one bug".into(),
        ));
    }
    Ok(())
}

pub(super) fn validate_args(args: &UpdateArgs) -> Result<()> {
    validate_draft(&BugUpdateDraft::from_cli(args), &args.ids)
}
