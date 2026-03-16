use crate::cli::GroupAction;
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
            let mut updates = serde_json::Map::new();
            if let Some(d) = description {
                updates.insert("description".into(), serde_json::Value::String(d.clone()));
            }
            if let Some(a) = is_active {
                updates.insert("is_active".into(), serde_json::Value::Bool(*a));
            }
            let body = serde_json::Value::Object(updates);
            client.update_group(group, &body).await?;
            #[expect(clippy::print_stdout)]
            {
                println!("Updated group '{group}'");
            }
        }
    }
    Ok(())
}
