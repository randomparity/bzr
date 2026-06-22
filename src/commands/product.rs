use crate::cli::ProductAction;
use crate::error::{BzrError, Result};
use crate::output::resources::product::{write_product_detail, write_products};
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateProductParams, UpdateProductParams};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonCreateProduct {
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    is_open: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonUpdateProduct {
    name: Option<String>,
    description: Option<String>,
    default_milestone: Option<String>,
    is_open: Option<bool>,
}

pub async fn execute(
    action: &ProductAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        ProductAction::List { r#type } => {
            let client = super::runtime::shared::connect_and_configure(server, api).await?;
            let products = client.list_products_by_type(*r#type).await?;
            write_products(&products, format, w.out);
        }
        ProductAction::View { name } => {
            let client = super::runtime::shared::connect_and_configure(server, api).await?;
            let product = client.get_product(name).await?;
            write_product_detail(&product, format, w.out);
        }
        ProductAction::Create {
            from_json,
            name,
            description,
            version,
            is_open,
        } => {
            let params = build_create_params(
                from_json.as_deref(),
                name.as_deref(),
                description.as_deref(),
                version.as_deref(),
                *is_open,
            )?;
            if super::runtime::dry_run::enabled() {
                let message = format!("Would create product '{}'", params.name);
                write_result(
                    &DryRunResult::new(ResourceKind::Product, &[], &params),
                    &message,
                    format,
                    w.out,
                );
                return Ok(());
            }
            let client = super::runtime::shared::connect_and_configure(server, api).await?;
            let id = client.create_product(&params).await?;
            write_result(
                &ActionResult::created_named(id, params.name.as_str(), ResourceKind::Product),
                &format!("Created product #{id} '{}'", params.name),
                format,
                w.out,
            );
        }
        ProductAction::Update {
            from_json,
            name,
            description,
            default_milestone,
            is_open,
        } => {
            let (name, params) = build_update_params(
                from_json.as_deref(),
                name.as_deref(),
                description.as_deref(),
                default_milestone.as_deref(),
                *is_open,
            )?;
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
            let client = super::runtime::shared::connect_and_configure(server, api).await?;
            client.update_product(&name, &params).await?;
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

pub(crate) fn requires_credentials(action: &ProductAction) -> Option<&'static str> {
    match action {
        ProductAction::List { .. } | ProductAction::View { .. } => None,
        ProductAction::Create { .. } => Some("product create"),
        ProductAction::Update { .. } => Some("product update"),
    }
}

fn build_create_params(
    from_json: Option<&str>,
    name: Option<&str>,
    description: Option<&str>,
    version: Option<&str>,
    is_open: Option<bool>,
) -> Result<CreateProductParams> {
    let mut input = if let Some(arg) = from_json {
        super::runtime::from_json::read_object::<JsonCreateProduct>(arg)?
    } else {
        JsonCreateProduct::default()
    };
    super::runtime::from_json::merge_string(&mut input.name, name);
    super::runtime::from_json::merge_string(&mut input.description, description);
    super::runtime::from_json::merge_string(&mut input.version, version);
    super::runtime::from_json::merge_copy(&mut input.is_open, is_open);
    Ok(CreateProductParams {
        name: super::runtime::from_json::required_string(input.name, "name")?,
        description: super::runtime::from_json::required_string(input.description, "description")?,
        version: input.version.unwrap_or_else(|| "unspecified".to_string()),
        is_open: input.is_open.unwrap_or(true),
    })
}

fn build_update_params(
    from_json: Option<&str>,
    name: Option<&str>,
    description: Option<&str>,
    default_milestone: Option<&str>,
    is_open: Option<bool>,
) -> Result<(String, UpdateProductParams)> {
    let mut input = if let Some(arg) = from_json {
        super::runtime::from_json::read_object::<JsonUpdateProduct>(arg)?
    } else {
        JsonUpdateProduct::default()
    };
    let target = super::runtime::from_json::resolve_string_target(
        name,
        input.name.take(),
        "--from-json object cannot combine positional product name with JSON name",
        "--from-json object requires a product name",
    )?;
    super::runtime::from_json::merge_string(&mut input.description, description);
    super::runtime::from_json::merge_string(&mut input.default_milestone, default_milestone);
    super::runtime::from_json::merge_copy(&mut input.is_open, is_open);
    let params = UpdateProductParams {
        description: input.description,
        default_milestone: input.default_milestone,
        is_open: input.is_open,
    };
    validate_update_params(&params)?;
    Ok((target, params))
}

fn validate_update_params(params: &UpdateProductParams) -> Result<()> {
    if params.description.is_none()
        && params.default_milestone.is_none()
        && params.is_open.is_none()
    {
        return Err(BzrError::InputValidation(
            "no fields to update; specify at least one field to change".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "product_tests.rs"]
mod tests;
