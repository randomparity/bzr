use crate::cli::ComponentAction;
use crate::error::{BzrError, Result};
use crate::output::resources::component::{write_component, write_components};
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
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
    validate_action(action)?;
    let client = super::runtime::shared::connect_and_configure(server, api).await?;

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
            if super::runtime::dry_run::enabled() {
                let message = format!("Would create component '{name}' in product '{product}'");
                write_result(
                    &DryRunResult::new(ResourceKind::Component, &[], &params),
                    &message,
                    format,
                    w.out,
                );
                return Ok(());
            }
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
            if super::runtime::dry_run::enabled() {
                let ids = [*id];
                let message = format!("Would update component #{id}");
                write_result(
                    &DryRunResult::new(ResourceKind::Component, &ids, &params),
                    &message,
                    format,
                    w.out,
                );
                return Ok(());
            }
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

#[must_use]
pub fn is_dry_runnable(action: &ComponentAction) -> bool {
    matches!(
        action,
        ComponentAction::Create { .. } | ComponentAction::Update { .. }
    )
}

fn validate_action(action: &ComponentAction) -> Result<()> {
    if let ComponentAction::Update {
        name,
        description,
        default_assignee,
        ..
    } = action
    {
        if name.is_none() && description.is_none() && default_assignee.is_none() {
            return Err(BzrError::InputValidation(
                "no fields to update; specify at least one field to change".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
