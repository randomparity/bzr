use serde::Deserialize;

use crate::commands::runtime::context::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::product::UpdateProductParams;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonUpdateProduct {
    name: Option<String>,
    description: Option<String>,
    default_milestone: Option<String>,
    is_open: Option<bool>,
}

pub(super) struct UpdateArgs<'a> {
    pub(super) from_json: Option<&'a str>,
    pub(super) name: Option<&'a str>,
    pub(super) description: Option<&'a str>,
    pub(super) default_milestone: Option<&'a str>,
    pub(super) is_open: Option<bool>,
}

pub(super) async fn handle(
    args: &UpdateArgs<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let (name, params) = build_params(args)?;
    let format = ctx.format();
    if ctx.dry_run() {
        let message = format!("Would update product '{name}'");
        write_result(
            &DryRunResult::new(ResourceKind::Product, &[], &params),
            &message,
            format,
            w.out,
        );
        return Ok(());
    }
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    client.update_product(&name, &params).await?;
    write_result(
        &ActionResult::updated_named(name.as_str(), None, ResourceKind::Product),
        &format!("Updated product '{name}'"),
        format,
        w.out,
    );
    Ok(())
}

fn build_params(args: &UpdateArgs<'_>) -> Result<(String, UpdateProductParams)> {
    let mut input = if let Some(arg) = args.from_json {
        crate::commands::runtime::from_json::read_object::<JsonUpdateProduct>(arg)?
    } else {
        JsonUpdateProduct::default()
    };
    let target = crate::commands::runtime::from_json::resolve_string_target(
        args.name,
        input.name.take(),
        "--from-json object cannot combine positional product name with JSON name",
        "--from-json object requires a product name",
    )?;
    crate::commands::runtime::from_json::merge_string(&mut input.description, args.description);
    crate::commands::runtime::from_json::merge_string(
        &mut input.default_milestone,
        args.default_milestone,
    );
    crate::commands::runtime::from_json::merge_copy(&mut input.is_open, args.is_open);
    let params = UpdateProductParams {
        description: input.description,
        default_milestone: input.default_milestone,
        is_open: input.is_open,
    };
    validate_params(&params)?;
    Ok((target, params))
}

fn validate_params(params: &UpdateProductParams) -> Result<()> {
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
