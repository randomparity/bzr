use crate::cli::GroupAction;
use crate::error::Result;
use crate::output::{self, ActionResult, MembershipResult, ResourceKind, Writers};
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateGroupParams, UpdateGroupParams};

pub async fn execute(
    action: &GroupAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    let client = super::shared::connect_and_configure(server, api).await?;

    match action {
        GroupAction::AddUser { group, user } => {
            client.add_user_to_group(user, group).await?;
            output::write_result(
                &MembershipResult::added(user.as_str(), group.as_str()),
                &format!("Added {user} to group '{group}'"),
                format,
                w.out,
            );
        }
        GroupAction::RemoveUser { group, user } => {
            client.remove_user_from_group(user, group).await?;
            output::write_result(
                &MembershipResult::removed(user.as_str(), group.as_str()),
                &format!("Removed {user} from group '{group}'"),
                format,
                w.out,
            );
        }
        GroupAction::ListUsers { group, details } => {
            let users = client.get_group_members(group, *details).await?;
            if *details {
                output::write_users_detailed(&users, format, w.out);
            } else {
                output::write_users(&users, format, w.out);
            }
        }
        GroupAction::View { group } => {
            let info = client.get_group(group).await?;
            output::write_group_info(&info, format, w.out);
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
            output::write_result(
                &ActionResult::created_named(id, name.as_str(), ResourceKind::Group),
                &format!("Created group #{id} '{name}'"),
                format,
                w.out,
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
            output::write_result(
                &ActionResult::updated_named(group.as_str(), None, ResourceKind::Group),
                &format!("Updated group '{group}'"),
                format,
                w.out,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "group_tests.rs"]
mod tests;
