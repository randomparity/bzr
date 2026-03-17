use crate::cli::ComponentAction;
use crate::client::UpdateComponentParams;
use crate::error::Result;
use crate::output::OutputFormat;

pub async fn execute(
    action: &ComponentAction,
    server: Option<&str>,
    _format: OutputFormat,
) -> Result<()> {
    let client = super::shared::build_client(server).await?;

    match action {
        ComponentAction::Create {
            product,
            name,
            description,
            default_assignee,
        } => {
            let id = client
                .create_component(product, name, description, default_assignee)
                .await?;
            #[expect(clippy::print_stdout)]
            {
                println!("Created component #{id} in product '{product}'");
            }
        }
        ComponentAction::Update {
            id,
            name,
            description,
            default_assignee,
        } => {
            let params = UpdateComponentParams {
                name: name.clone(),
                description: description.clone(),
                default_assignee: default_assignee.clone(),
            };
            client.update_component(*id, &params).await?;
            #[expect(clippy::print_stdout)]
            {
                println!("Updated component #{id}");
            }
        }
    }
    Ok(())
}
