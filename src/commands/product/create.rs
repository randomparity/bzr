use serde::Deserialize;

use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::CreateProductParams;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonCreateProduct {
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    is_open: Option<bool>,
}

pub(super) struct CreateArgs<'a> {
    pub(super) from_json: Option<&'a str>,
    pub(super) name: Option<&'a str>,
    pub(super) description: Option<&'a str>,
    pub(super) version: Option<&'a str>,
    pub(super) is_open: Option<bool>,
}

pub(super) async fn handle(
    args: CreateArgs<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let params = build_params(
        args.from_json,
        args.name,
        args.description,
        args.version,
        args.is_open,
    )?;
    let format = ctx.format();
    if ctx.dry_run() {
        let message = format!("Would create product '{}'", params.name);
        write_result(
            &DryRunResult::new(ResourceKind::Product, &[], &params),
            &message,
            format,
            w.out,
        );
        return Ok(());
    }
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let id = client.create_product(&params).await?;
    write_result(
        &ActionResult::created_named(id, params.name.as_str(), ResourceKind::Product),
        &format!("Created product #{id} '{}'", params.name),
        format,
        w.out,
    );
    Ok(())
}

fn build_params(
    from_json: Option<&str>,
    name: Option<&str>,
    description: Option<&str>,
    version: Option<&str>,
    is_open: Option<bool>,
) -> Result<CreateProductParams> {
    let mut input = if let Some(arg) = from_json {
        crate::commands::runtime::from_json::read_object::<JsonCreateProduct>(arg)?
    } else {
        JsonCreateProduct::default()
    };
    crate::commands::runtime::from_json::merge_string(&mut input.name, name);
    crate::commands::runtime::from_json::merge_string(&mut input.description, description);
    crate::commands::runtime::from_json::merge_string(&mut input.version, version);
    crate::commands::runtime::from_json::merge_copy(&mut input.is_open, is_open);
    Ok(CreateProductParams {
        name: crate::commands::runtime::from_json::required_string(input.name, "name")?,
        description: crate::commands::runtime::from_json::required_string(
            input.description,
            "description",
        )?,
        version: input.version.unwrap_or_else(|| "unspecified".to_string()),
        is_open: input.is_open.unwrap_or(true),
    })
}
