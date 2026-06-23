use crate::cli::ComponentAction;
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::writers::Writers;

mod create;
mod list;
mod update;
mod view;

pub async fn execute(
    action: &ComponentAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        ComponentAction::List { product } => list::handle(product, ctx, w).await,
        ComponentAction::View { product, name } => view::handle(product, name, ctx, w).await,
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
pub fn is_dry_runnable(action: &ComponentAction) -> bool {
    matches!(
        action,
        ComponentAction::Create { .. } | ComponentAction::Update { .. }
    )
}

pub(crate) fn requires_credentials(action: &ComponentAction) -> Option<&'static str> {
    match action {
        ComponentAction::List { .. } | ComponentAction::View { .. } => None,
        ComponentAction::Create { .. } => Some("component create"),
        ComponentAction::Update { .. } => Some("component update"),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
