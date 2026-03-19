use crate::cli::GroupAction;
use crate::config::ApiMode;
use crate::error::Result;
use crate::output::{self, OutputFormat};
use crate::types::{CreateGroupParams, UpdateGroupParams};

pub async fn execute(
    action: &GroupAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::build_client(server, api).await?;

    match action {
        GroupAction::AddUser { group, user } => {
            client.add_user_to_group(user, group).await?;
            output::print_result(
                &serde_json::json!({
                    "user": user,
                    "group": group,
                    "resource": "group_membership",
                    "action": "added",
                }),
                &format!("Added {user} to group '{group}'"),
                format,
            );
        }
        GroupAction::RemoveUser { group, user } => {
            client.remove_user_from_group(user, group).await?;
            output::print_result(
                &serde_json::json!({
                    "user": user,
                    "group": group,
                    "resource": "group_membership",
                    "action": "removed",
                }),
                &format!("Removed {user} from group '{group}'"),
                format,
            );
        }
        GroupAction::ListUsers { group, details } => {
            let users = client.get_group_members(group, *details).await?;
            output::print_users(&users, format, *details);
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
                &serde_json::json!({
                    "id": id,
                    "name": name,
                    "resource": "group",
                    "action": "created",
                }),
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
                &serde_json::json!({
                    "name": group,
                    "resource": "group",
                    "action": "updated",
                }),
                &format!("Updated group '{group}'"),
                format,
            );
        }
    }
    Ok(())
}
