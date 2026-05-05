use crate::cli::ComponentAction;
use crate::error::Result;
use crate::output::{self, ActionResult, ResourceKind};
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateComponentParams, UpdateComponentParams};

pub async fn execute(
    action: &ComponentAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_and_configure(server, api).await?;

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
                &ActionResult::created(id, ResourceKind::Component),
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
                &ActionResult::updated(*id, ResourceKind::Component),
                &format!("Updated component #{id}"),
                format,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
