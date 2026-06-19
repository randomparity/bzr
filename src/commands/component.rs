use crate::cli::ComponentAction;
use crate::error::{BzrError, Result};
use crate::output::resources::component::{write_component, write_components};
use crate::output::result_types::{write_result, ActionResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateComponentParams, UpdateComponentParams};

pub async fn execute(
    action: &ComponentAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    let client = super::shared::connect_and_configure(server, api).await?;

    match action {
        ComponentAction::List { product } => {
            let product = client.get_product(product).await?;
            write_components(&product.components, format, w.out);
        }
        ComponentAction::View { product, name } => {
            let product = client.get_product(product).await?;
            let component = product
                .components
                .iter()
                .find(|c| c.name == *name)
                .ok_or_else(|| BzrError::NotFound {
                    resource: "component",
                    id: name.clone(),
                })?;
            write_component(component, format, w.out);
        }
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
            write_result(
                &ActionResult::created(id, ResourceKind::Component),
                &format!("Created component #{id} in product '{product}'"),
                format,
                w.out,
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
            write_result(
                &ActionResult::updated(*id, ResourceKind::Component),
                &format!("Updated component #{id}"),
                format,
                w.out,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
