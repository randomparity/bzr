use crate::cli::ComponentAction;
use crate::error::{BzrError, Result};
use crate::output::resources::component::{write_component, write_components};
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateComponentParams, UpdateComponentParams};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonCreateComponent {
    product: Option<String>,
    name: Option<String>,
    description: Option<String>,
    default_assignee: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonUpdateComponent {
    id: Option<u64>,
    name: Option<String>,
    description: Option<String>,
    default_assignee: Option<String>,
}

pub async fn execute(
    action: &ComponentAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        ComponentAction::List { product } => {
            let client = super::runtime::shared::connect_and_configure(server, api).await?;
            let product = client.get_product(product).await?;
            write_components(&product.components, format, w.out);
        }
        ComponentAction::View { product, name } => {
            let client = super::runtime::shared::connect_and_configure(server, api).await?;
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
            from_json,
            product,
            name,
            description,
            default_assignee,
        } => {
            let params = build_create_params(
                from_json.as_deref(),
                product.as_deref(),
                name.as_deref(),
                description.as_deref(),
                default_assignee.as_deref(),
            )?;
            if super::runtime::dry_run::enabled() {
                let message = format!(
                    "Would create component '{}' in product '{}'",
                    params.name, params.product
                );
                write_result(
                    &DryRunResult::new(ResourceKind::Component, &[], &params),
                    &message,
                    format,
                    w.out,
                );
                return Ok(());
            }
            let client = super::runtime::shared::connect_and_configure(server, api).await?;
            let id = client.create_component(&params).await?;
            write_result(
                &ActionResult::created(id, ResourceKind::Component),
                &format!("Created component #{id} in product '{}'", params.product),
                format,
                w.out,
            );
        }
        ComponentAction::Update {
            from_json,
            id,
            name,
            description,
            default_assignee,
        } => {
            let (id, params) = build_update_params(
                from_json.as_deref(),
                *id,
                name.as_deref(),
                description.as_deref(),
                default_assignee.as_deref(),
            )?;
            if super::runtime::dry_run::enabled() {
                let ids = [id];
                let message = format!("Would update component #{id}");
                write_result(
                    &DryRunResult::new(ResourceKind::Component, &ids, &params),
                    &message,
                    format,
                    w.out,
                );
                return Ok(());
            }
            let client = super::runtime::shared::connect_and_configure(server, api).await?;
            client.update_component(id, &params).await?;
            write_result(
                &ActionResult::updated(id, ResourceKind::Component),
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

fn build_create_params(
    from_json: Option<&str>,
    product: Option<&str>,
    name: Option<&str>,
    description: Option<&str>,
    default_assignee: Option<&str>,
) -> Result<CreateComponentParams> {
    let mut input = if let Some(arg) = from_json {
        super::runtime::from_json::read_object::<JsonCreateComponent>(arg)?
    } else {
        JsonCreateComponent::default()
    };
    super::runtime::from_json::merge_string(&mut input.product, product);
    super::runtime::from_json::merge_string(&mut input.name, name);
    super::runtime::from_json::merge_string(&mut input.description, description);
    super::runtime::from_json::merge_string(&mut input.default_assignee, default_assignee);
    Ok(CreateComponentParams {
        product: super::runtime::from_json::required_string(input.product, "product")?,
        name: super::runtime::from_json::required_string(input.name, "name")?,
        description: super::runtime::from_json::required_string(input.description, "description")?,
        default_assignee: super::runtime::from_json::required_string(
            input.default_assignee,
            "default_assignee",
        )?,
    })
}

fn build_update_params(
    from_json: Option<&str>,
    id: Option<u64>,
    name: Option<&str>,
    description: Option<&str>,
    default_assignee: Option<&str>,
) -> Result<(u64, UpdateComponentParams)> {
    let mut input = if let Some(arg) = from_json {
        super::runtime::from_json::read_object::<JsonUpdateComponent>(arg)?
    } else {
        JsonUpdateComponent::default()
    };
    let target = super::runtime::from_json::resolve_u64_target(
        id,
        input.id.take(),
        "--from-json object cannot combine positional component ID with JSON id",
        "--from-json object requires a component id",
    )?;
    super::runtime::from_json::merge_string(&mut input.name, name);
    super::runtime::from_json::merge_string(&mut input.description, description);
    super::runtime::from_json::merge_string(&mut input.default_assignee, default_assignee);
    let params = UpdateComponentParams {
        name: input.name,
        description: input.description,
        default_assignee: input.default_assignee,
    };
    validate_update_params(&params)?;
    Ok((target, params))
}

fn validate_update_params(params: &UpdateComponentParams) -> Result<()> {
    if params.name.is_none() && params.description.is_none() && params.default_assignee.is_none() {
        return Err(BzrError::InputValidation(
            "no fields to update; specify at least one field to change".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
