use crate::cli::{BugAction, UpdateArgs};
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::writers::Writers;

mod draft;
mod execute;
mod output;
mod payload;
mod validate;

pub(super) use crate::commands::runtime::mutation::ensure_batch_complete;
pub(super) use draft::BugUpdateDraft;
pub(super) use execute::{
    apply_checked, apply_checked_connected, confirm_batch, ensure_unchanged_since, ApplyRequest,
};
pub(super) use output::write_batch_result;
use payload::build_update_params;
pub(super) use payload::{build_update_params_from_draft, resolve_comment};
use validate::validate_args;

pub(super) fn validate_action(action: &BugAction) -> Result<()> {
    match action {
        BugAction::Update(args) => validate_args(args),
        _ => Ok(()),
    }
}

pub(super) async fn handle(
    args: &UpdateArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    if let Some(arg) = args.from_json.as_deref() {
        return super::update_json::handle(args, arg, ctx, w).await;
    }
    let (ids, params) = build_update_params(args)?;
    apply_checked(
        ApplyRequest {
            ids,
            params,
            expect_unchanged_since: args.expect_unchanged_since.as_deref(),
        },
        ctx,
        w,
    )
    .await
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
