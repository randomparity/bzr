use crate::cli::ComponentAction;
use crate::config::ApiMode;
use crate::error::Result;
use crate::output;
use crate::types::OutputFormat;
use crate::types::{CreateComponentParams, UpdateComponentParams};

pub async fn execute(
    action: &ComponentAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::build_client(server, api).await?;

    match action {
        ComponentAction::Create {
            product,
            name,
            description,
            default_assignee,
        } => {
            let params = CreateComponentParams {
                product: product.clone(),
                name: name.clone(),
                description: description.clone(),
                default_assignee: default_assignee.clone(),
            };
            let id = client.create_component(&params).await?;
            output::print_result(
                &serde_json::json!({
                    "id": id,
                    "product": product,
                    "resource": "component",
                    "action": "created",
                }),
                &format!("Created component #{id} in product '{product}'"),
                format,
            );
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
            output::print_result(
                &serde_json::json!({
                    "id": id,
                    "resource": "component",
                    "action": "updated",
                }),
                &format!("Updated component #{id}"),
                format,
            );
        }
    }
    Ok(())
}
