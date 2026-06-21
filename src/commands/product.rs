use crate::cli::ProductAction;
use crate::error::{BzrError, Result};
use crate::output::resources::product::{write_product_detail, write_products};
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateProductParams, UpdateProductParams};

pub async fn execute(
    action: &ProductAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    validate_action(action)?;
    let client = super::runtime::shared::connect_and_configure(server, api).await?;

    match action {
        ProductAction::List { r#type } => {
            let products = client.list_products_by_type(*r#type).await?;
            write_products(&products, format, w.out);
        }
        ProductAction::View { name } => {
            let product = client.get_product(name).await?;
            write_product_detail(&product, format, w.out);
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
            if super::runtime::dry_run::enabled() {
                let message = format!("Would create product '{name}'");
                write_result(
                    &DryRunResult::new(ResourceKind::Product, &[], &params),
                    &message,
                    format,
                    w.out,
                );
                return Ok(());
            }
            let id = client.create_product(&params).await?;
            write_result(
                &ActionResult::created_named(id, name.as_str(), ResourceKind::Product),
                &format!("Created product #{id} '{name}'"),
                format,
                w.out,
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
            if super::runtime::dry_run::enabled() {
                let message = format!("Would update product '{name}'");
                write_result(
                    &DryRunResult::new(ResourceKind::Product, &[], &params),
                    &message,
                    format,
                    w.out,
                );
                return Ok(());
            }
            client.update_product(name, &params).await?;
            write_result(
                &ActionResult::updated_named(name.as_str(), None, ResourceKind::Product),
                &format!("Updated product '{name}'"),
                format,
                w.out,
            );
        }
    }
    Ok(())
}

#[must_use]
pub fn is_dry_runnable(action: &ProductAction) -> bool {
    matches!(
        action,
        ProductAction::Create { .. } | ProductAction::Update { .. }
    )
}

fn validate_action(action: &ProductAction) -> Result<()> {
    if let ProductAction::Update {
        description,
        default_milestone,
        is_open,
        ..
    } = action
    {
        if description.is_none() && default_milestone.is_none() && is_open.is_none() {
            return Err(BzrError::InputValidation(
                "no fields to update; specify at least one field to change".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "product_tests.rs"]
mod tests;
