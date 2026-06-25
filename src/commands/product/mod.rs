//! Product subcommand handlers, split per action.

use crate::cli::ProductAction;
use crate::commands::runtime::capabilities::CommandCapabilities;
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::writers::Writers;

mod create;
mod list;
mod update;
mod view;

pub(crate) async fn execute(
    action: &ProductAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        ProductAction::List { r#type } => list::handle(*r#type, ctx, w).await,
        ProductAction::View { name } => view::handle(name, ctx, w).await,
        ProductAction::Create {
            from_json,
            name,
            description,
            version,
            is_open,
        } => {
            let args = create::CreateArgs {
                from_json: from_json.as_deref(),
                name: name.as_deref(),
                description: description.as_deref(),
                version: version.as_deref(),
                is_open: *is_open,
            };
            create::handle(&args, ctx, w).await
        }
        ProductAction::Update {
            from_json,
            name,
            description,
            default_milestone,
            is_open,
        } => {
            let args = update::UpdateArgs {
                from_json: from_json.as_deref(),
                name: name.as_deref(),
                description: description.as_deref(),
                default_milestone: default_milestone.as_deref(),
                is_open: *is_open,
            };
            update::handle(&args, ctx, w).await
        }
    }
}

#[must_use]
pub(crate) fn capabilities(action: &ProductAction) -> CommandCapabilities {
    match action {
        ProductAction::List { .. } | ProductAction::View { .. } => CommandCapabilities::anonymous(),
        ProductAction::Create { .. } => CommandCapabilities::dry_run_mutation("product create"),
        ProductAction::Update { .. } => CommandCapabilities::dry_run_mutation("product update"),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
