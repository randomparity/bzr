use serde::Deserialize;

use crate::commands::runtime::invocation::CommandContext;
use crate::commands::runtime::mutation::{self, Committed, DryRunPreview};
use crate::error::Result;
use crate::output::result_types::{ActionResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::product::CreateProductParams;

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
    args: &CreateArgs<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let params = build_params(args)?;
    let message = format!("Would create product '{}'", params.name);
    mutation::run(
        ctx,
        w,
        DryRunPreview {
            resource: ResourceKind::Product,
            params,
            message,
        },
        |client, params| async move {
            let id = client.create_product(&params).await?;
            Ok(Committed {
                result: ActionResult::created_named(
                    id,
                    params.name.as_str(),
                    ResourceKind::Product,
                ),
                message: format!("Created product #{id} '{}'", params.name),
            })
        },
    )
    .await
}

fn build_params(args: &CreateArgs<'_>) -> Result<CreateProductParams> {
    let mut input = if let Some(arg) = args.from_json {
        crate::commands::runtime::input::from_json::read_object::<JsonCreateProduct>(arg)?
    } else {
        JsonCreateProduct::default()
    };
    crate::commands::runtime::input::from_json::merge_string(&mut input.name, args.name);
    crate::commands::runtime::input::from_json::merge_string(
        &mut input.description,
        args.description,
    );
    crate::commands::runtime::input::from_json::merge_string(&mut input.version, args.version);
    crate::commands::runtime::input::from_json::merge_copy(&mut input.is_open, args.is_open);
    Ok(CreateProductParams {
        name: crate::commands::runtime::input::from_json::required_string(input.name, "name")?,
        description: crate::commands::runtime::input::from_json::required_string(
            input.description,
            "description",
        )?,
        version: input.version.unwrap_or_else(|| "unspecified".to_string()),
        is_open: input.is_open.unwrap_or(true),
    })
}

#[cfg(test)]
#[path = "create_tests.rs"]
mod tests;
