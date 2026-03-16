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
    }
    Ok(())
}
