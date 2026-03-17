use crate::cli::GroupAction;
use crate::client::UpdateGroupParams;
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn execute(
    action: &GroupAction,
    server: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let client = super::shared::build_client(server).await?;

    match action {
        GroupAction::AddUser { group, user } => {
            client.add_user_to_group(user, group).await?;
            #[expect(clippy::print_stdout)]
            {
                println!("Added {user} to group '{group}'");
            }
        }
        GroupAction::RemoveUser { group, user } => {
            client.remove_user_from_group(user, group).await?;
            #[expect(clippy::print_stdout)]
            {
                println!("Removed {user} from group '{group}'");
            }
        }
        GroupAction::ListUsers { group } => {
            let users = client.get_group_members(group).await?;
            output::print_users(&users, format);
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
            let id = client.create_group(name, description, *is_active).await?;
            #[expect(clippy::print_stdout)]
            {
                println!("Created group #{id} '{name}'");
            }
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
            #[expect(clippy::print_stdout)]
            {
                println!("Updated group '{group}'");
            }
        }
    }
    Ok(())
}
