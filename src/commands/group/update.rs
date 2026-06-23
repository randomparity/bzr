use serde::Deserialize;

use crate::commands::runtime::context::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::UpdateGroupParams;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonUpdateGroup {
    group: Option<String>,
    description: Option<String>,
    is_active: Option<bool>,
}

pub(super) struct UpdateArgs<'a> {
    pub(super) from_json: Option<&'a str>,
    pub(super) group: Option<&'a str>,
    pub(super) description: Option<&'a str>,
    pub(super) is_active: Option<bool>,
}

pub(super) async fn handle(
    args: UpdateArgs<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let (group, params) =
        build_params(args.from_json, args.group, args.description, args.is_active)?;
    let format = ctx.format();
    if ctx.dry_run() {
        let message = format!("Would update group '{group}'");
        write_result(
            &DryRunResult::new(ResourceKind::Group, &[], &params),
            &message,
            format,
            w.out,
        );
        return Ok(());
    }
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    client.update_group(&group, &params).await?;
    write_result(
        &ActionResult::updated_named(group.as_str(), None, ResourceKind::Group),
        &format!("Updated group '{group}'"),
        format,
        w.out,
    );
    Ok(())
}

fn build_params(
    from_json: Option<&str>,
    group: Option<&str>,
    description: Option<&str>,
    is_active: Option<bool>,
) -> Result<(String, UpdateGroupParams)> {
    let mut input = if let Some(arg) = from_json {
        crate::commands::runtime::from_json::read_object::<JsonUpdateGroup>(arg)?
    } else {
        JsonUpdateGroup::default()
    };
    let target = crate::commands::runtime::from_json::resolve_string_target(
        group,
        input.group.take(),
        "--from-json object cannot combine positional group with JSON group",
        "--from-json object requires a group",
    )?;
    crate::commands::runtime::from_json::merge_string(&mut input.description, description);
    crate::commands::runtime::from_json::merge_copy(&mut input.is_active, is_active);
    let params = UpdateGroupParams {
        description: input.description,
        is_active: input.is_active,
    };
    validate_params(&params)?;
    Ok((target, params))
}

fn validate_params(params: &UpdateGroupParams) -> Result<()> {
    if params.description.is_none() && params.is_active.is_none() {
        return Err(BzrError::InputValidation(
            "no fields to update; specify at least one field to change".into(),
        ));
    }
    Ok(())
}
