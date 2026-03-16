use crate::cli::UserAction;
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn execute(
    action: &UserAction,
    server: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let client = super::shared::build_client(server).await?;

    match action {
        UserAction::Search { query } => {
            let users = client.search_users(query).await?;
            output::print_users(&users, format);
        }
    }
    Ok(())
}
