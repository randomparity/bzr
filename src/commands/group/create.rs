use serde::Deserialize;

use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::CreateGroupParams;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonCreateGroup {
    name: Option<String>,
    description: Option<String>,
    is_active: Option<bool>,
}

pub(super) struct CreateArgs<'a> {
    pub(super) from_json: Option<&'a str>,
    pub(super) name: Option<&'a str>,
    pub(super) description: Option<&'a str>,
    pub(super) is_active: Option<bool>,
}

pub(super) async fn handle(
    args: &CreateArgs<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let params = build_params(args)?;
    let format = ctx.format();
    if ctx.dry_run() {
        let message = format!("Would create group '{}'", params.name);
        write_result(
            &DryRunResult::new(ResourceKind::Group, &[], &params),
            &message,
            format,
            w.out,
        );
        return Ok(());
    }
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let id = client.create_group(&params).await?;
    write_result(
        &ActionResult::created_named(id, params.name.as_str(), ResourceKind::Group),
        &format!("Created group #{id} '{}'", params.name),
        format,
        w.out,
    );
    Ok(())
}

fn build_params(args: &CreateArgs<'_>) -> Result<CreateGroupParams> {
    let mut input = if let Some(arg) = args.from_json {
        crate::commands::runtime::from_json::read_object::<JsonCreateGroup>(arg)?
    } else {
        JsonCreateGroup::default()
    };
    crate::commands::runtime::from_json::merge_string(&mut input.name, args.name);
    crate::commands::runtime::from_json::merge_string(&mut input.description, args.description);
    crate::commands::runtime::from_json::merge_copy(&mut input.is_active, args.is_active);
    Ok(CreateGroupParams {
        name: crate::commands::runtime::from_json::required_string(input.name, "name")?,
        description: crate::commands::runtime::from_json::required_string(
            input.description,
            "description",
        )?,
        is_active: input.is_active.unwrap_or(true),
    })
}
