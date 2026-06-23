use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::component::CreateComponentParams;
use serde::Deserialize;

pub(super) struct CreateArgs<'a> {
    pub(super) from_json: Option<&'a str>,
    pub(super) product: Option<&'a str>,
    pub(super) name: Option<&'a str>,
    pub(super) description: Option<&'a str>,
    pub(super) default_assignee: Option<&'a str>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonCreateComponent {
    product: Option<String>,
    name: Option<String>,
    description: Option<String>,
    default_assignee: Option<String>,
}

pub(super) async fn handle(
    args: &CreateArgs<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    let params = build_create_params(args)?;
    if ctx.dry_run() {
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

    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let id = client.create_component(&params).await?;
    write_result(
        &ActionResult::created(id, ResourceKind::Component),
        &format!("Created component #{id} in product '{}'", params.product),
        format,
        w.out,
    );
    Ok(())
}

fn build_create_params(args: &CreateArgs<'_>) -> Result<CreateComponentParams> {
    let mut input = if let Some(arg) = args.from_json {
        crate::commands::runtime::from_json::read_object::<JsonCreateComponent>(arg)?
    } else {
        JsonCreateComponent::default()
    };
    crate::commands::runtime::from_json::merge_string(&mut input.product, args.product);
    crate::commands::runtime::from_json::merge_string(&mut input.name, args.name);
    crate::commands::runtime::from_json::merge_string(&mut input.description, args.description);
    crate::commands::runtime::from_json::merge_string(
        &mut input.default_assignee,
        args.default_assignee,
    );
    Ok(CreateComponentParams {
        product: crate::commands::runtime::from_json::required_string(input.product, "product")?,
        name: crate::commands::runtime::from_json::required_string(input.name, "name")?,
        description: crate::commands::runtime::from_json::required_string(
            input.description,
            "description",
        )?,
        default_assignee: crate::commands::runtime::from_json::required_string(
            input.default_assignee,
            "default_assignee",
        )?,
    })
}

#[cfg(test)]
#[path = "create_tests.rs"]
mod tests;
