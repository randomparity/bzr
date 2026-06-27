//! Group subcommand handlers, split per action.

use crate::cli::GroupAction;
use crate::commands::runtime::capabilities::CommandCapabilities;
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::writers::Writers;

mod add_user;
mod create;
mod list_users;
mod remove_user;
mod update;
mod view;

pub(crate) async fn execute(
    action: &GroupAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        GroupAction::AddUser { group, user } => add_user::handle(group, user, ctx, w).await,
        GroupAction::RemoveUser { group, user } => remove_user::handle(group, user, ctx, w).await,
        GroupAction::ListUsers {
            group,
            details,
            projection,
        } => list_users::handle(group, *details, projection, ctx, w).await,
        GroupAction::View { group, projection } => view::handle(group, projection, ctx, w).await,
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
pub(crate) fn capabilities(action: &GroupAction) -> CommandCapabilities {
    match action {
        GroupAction::ListUsers { .. } | GroupAction::View { .. } => {
            CommandCapabilities::anonymous()
        }
        GroupAction::AddUser { .. } => CommandCapabilities::authenticated("group add-user"),
        GroupAction::RemoveUser { .. } => CommandCapabilities::authenticated("group remove-user"),
        GroupAction::Create { .. } => CommandCapabilities::dry_run_mutation("group create"),
        GroupAction::Update { .. } => CommandCapabilities::dry_run_mutation("group update"),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
