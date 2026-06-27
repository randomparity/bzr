use serde::Deserialize;

use crate::commands::runtime::invocation::CommandContext;
use crate::commands::runtime::mutation::{self, Committed, DryRunPreview};
use crate::error::Result;
use crate::output::result_types::{ActionResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::group::CreateGroupParams;

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
    let message = format!("Would create group '{}'", params.name);
    mutation::run(
        ctx,
        w,
        DryRunPreview {
            resource: ResourceKind::Group,
            params,
            message,
        },
        |client, params| async move {
            let id = client.create_group(&params).await?;
            Ok(Committed {
                result: ActionResult::created_named(id, params.name.as_str(), ResourceKind::Group),
                message: format!("Created group #{id} '{}'", params.name),
            })
        },
    )
    .await
}

fn build_params(args: &CreateArgs<'_>) -> Result<CreateGroupParams> {
    let mut input = if let Some(arg) = args.from_json {
        crate::commands::runtime::input::from_json::read_object::<JsonCreateGroup>(arg)?
    } else {
        JsonCreateGroup::default()
    };
    crate::commands::runtime::input::from_json::merge_string(&mut input.name, args.name);
    crate::commands::runtime::input::from_json::merge_string(
        &mut input.description,
        args.description,
    );
    crate::commands::runtime::input::from_json::merge_copy(&mut input.is_active, args.is_active);
    Ok(CreateGroupParams {
        name: crate::commands::runtime::input::from_json::required_string(input.name, "name")?,
        description: crate::commands::runtime::input::from_json::required_string(
            input.description,
            "description",
        )?,
        is_active: input.is_active.unwrap_or(true),
    })
}

#[cfg(test)]
#[path = "create_tests.rs"]
mod tests;
