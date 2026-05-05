use crate::cli::ProductAction;
use crate::error::Result;
use crate::output::{self, ActionResult, ResourceKind};
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateProductParams, UpdateProductParams};

pub async fn execute(
    action: &ProductAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_and_configure(server, api).await?;

    match action {
        ProductAction::List { r#type } => {
            let products = client.list_products_by_type(*r#type).await?;
            output::print_products(&products, format);
        }
        ProductAction::View { name } => {
            let product = client.get_product(name).await?;
            output::print_product_detail(&product, format);
        }
        ProductAction::Create {
            name,
            description,
            version,
            is_open,
        } => {
            let params = CreateProductParams {
                name: name.clone(),
                description: description.clone(),
                version: version.clone(),
                is_open: *is_open,
            };
            let id = client.create_product(&params).await?;
            output::print_result(
                &ActionResult::created_named(id, name.as_str(), ResourceKind::Product),
                &format!("Created product #{id} '{name}'"),
                format,
            );
        }
        ProductAction::Update {
            name,
            description,
            default_milestone,
            is_open,
        } => {
            let params = UpdateProductParams {
                description: description.clone(),
                default_milestone: default_milestone.clone(),
                is_open: *is_open,
            };
            client.update_product(name, &params).await?;
            output::print_result(
                &ActionResult::updated_named(name.as_str(), None, ResourceKind::Product),
                &format!("Updated product '{name}'"),
                format,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "product_tests.rs"]
mod tests;
