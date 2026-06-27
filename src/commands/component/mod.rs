use crate::cli::ComponentAction;
use crate::commands::runtime::capabilities::CommandCapabilities;
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::writers::Writers;

mod create;
mod list;
mod update;
mod view;

pub(crate) async fn execute(
    action: &ComponentAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        ComponentAction::List {
            product,
            projection,
        } => list::handle(product, projection, ctx, w).await,
        ComponentAction::View {
            product,
            name,
            projection,
        } => view::handle(product, name, projection, ctx, w).await,
        ComponentAction::Create {
            from_json,
            product,
            name,
            description,
            default_assignee,
        } => {
            let args = create::CreateArgs {
                from_json: from_json.as_deref(),
                product: product.as_deref(),
                name: name.as_deref(),
                description: description.as_deref(),
                default_assignee: default_assignee.as_deref(),
            };
            create::handle(&args, ctx, w).await
        }
        ComponentAction::Update {
            from_json,
            id,
            product,
            component,
            name,
            description,
            default_assignee,
        } => {
            let args = update::UpdateArgs {
                from_json: from_json.as_deref(),
                id: *id,
                product: product.as_deref(),
                component: component.as_deref(),
                name: name.as_deref(),
                description: description.as_deref(),
                default_assignee: default_assignee.as_deref(),
            };
            update::handle(&args, ctx, w).await
        }
    }
}

#[must_use]
pub(crate) fn capabilities(action: &ComponentAction) -> CommandCapabilities {
    match action {
        ComponentAction::List { .. } | ComponentAction::View { .. } => {
            CommandCapabilities::anonymous()
        }
        ComponentAction::Create { .. } => CommandCapabilities::dry_run_mutation("component create"),
        ComponentAction::Update { .. } => CommandCapabilities::dry_run_mutation("component update"),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
