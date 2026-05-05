use crate::cli::GroupAction;
use crate::error::Result;
use crate::output::{self, ActionResult, MembershipResult, ResourceKind};
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateGroupParams, UpdateGroupParams};

pub async fn execute(
    action: &GroupAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_and_configure(server, api).await?;

    match action {
        GroupAction::AddUser { group, user } => {
            client.add_user_to_group(user, group).await?;
            output::print_result(
                &MembershipResult::added(user.as_str(), group.as_str()),
                &format!("Added {user} to group '{group}'"),
                format,
            );
        }
        GroupAction::RemoveUser { group, user } => {
            client.remove_user_from_group(user, group).await?;
            output::print_result(
                &MembershipResult::removed(user.as_str(), group.as_str()),
                &format!("Removed {user} from group '{group}'"),
                format,
            );
        }
        GroupAction::ListUsers { group, details } => {
            let users = client.get_group_members(group, *details).await?;
            if *details {
                output::print_users_detailed(&users, format);
            } else {
                output::print_users(&users, format);
            }
        }
        GroupAction::View { group } => {
            let info = client.get_group(group).await?;
            output::print_group_info(&info, format);
        }
        GroupAction::Create {
            name,
            description,
            is_active,
        } => {
            let params = CreateGroupParams {
                name: name.clone(),
                description: description.clone(),
                is_active: *is_active,
            };
            let id = client.create_group(&params).await?;
            output::print_result(
                &ActionResult::created_named(id, name.as_str(), ResourceKind::Group),
                &format!("Created group #{id} '{name}'"),
                format,
            );
        }
        GroupAction::Update {
            group,
            description,
            is_active,
        } => {
            let params = UpdateGroupParams {
                description: description.clone(),
                is_active: *is_active,
            };
            client.update_group(group, &params).await?;
            output::print_result(
                &ActionResult::updated_named(group.as_str(), None, ResourceKind::Group),
                &format!("Updated group '{group}'"),
                format,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "group_tests.rs"]
mod tests;
