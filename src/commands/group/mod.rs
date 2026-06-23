//! Group subcommand handlers, split per action.

use crate::cli::GroupAction;
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::writers::Writers;

mod add_user;
mod create;
mod list_users;
mod remove_user;
mod update;
mod view;

pub async fn execute(
    action: &GroupAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        GroupAction::AddUser { group, user } => add_user::handle(group, user, ctx, w).await,
        GroupAction::RemoveUser { group, user } => remove_user::handle(group, user, ctx, w).await,
        GroupAction::ListUsers { group, details } => {
            list_users::handle(group, *details, ctx, w).await
        }
        GroupAction::View { group } => view::handle(group, ctx, w).await,
        GroupAction::Create {
            from_json,
            name,
            description,
            is_active,
        } => {
            let args = create::CreateArgs {
                from_json: from_json.as_deref(),
                name: name.as_deref(),
                description: description.as_deref(),
                is_active: *is_active,
            };
            create::handle(&args, ctx, w).await
        }
        GroupAction::Update {
            from_json,
            group,
            description,
            is_active,
        } => {
            let args = update::UpdateArgs {
                from_json: from_json.as_deref(),
                group: group.as_deref(),
                description: description.as_deref(),
                is_active: *is_active,
            };
            update::handle(&args, ctx, w).await
        }
    }
}

#[must_use]
pub fn is_dry_runnable(action: &GroupAction) -> bool {
    matches!(
        action,
        GroupAction::Create { .. } | GroupAction::Update { .. }
    )
}

pub(crate) fn requires_credentials(action: &GroupAction) -> Option<&'static str> {
    match action {
        GroupAction::ListUsers { .. } | GroupAction::View { .. } => None,
        GroupAction::AddUser { .. } => Some("group add-user"),
        GroupAction::RemoveUser { .. } => Some("group remove-user"),
        GroupAction::Create { .. } => Some("group create"),
        GroupAction::Update { .. } => Some("group update"),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
