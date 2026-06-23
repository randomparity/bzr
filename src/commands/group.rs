use crate::cli::GroupAction;
use crate::commands::runtime::context::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::resources::group::write_group_info;
use crate::output::resources::user::{write_users, write_users_detailed};
use crate::output::result_types::{
    write_result, ActionResult, DryRunResult, MembershipResult, ResourceKind,
};
use crate::output::writers::Writers;
use crate::types::{CreateGroupParams, UpdateGroupParams};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonCreateGroup {
    name: Option<String>,
    description: Option<String>,
    is_active: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonUpdateGroup {
    group: Option<String>,
    description: Option<String>,
    is_active: Option<bool>,
}

pub async fn execute(
    action: &GroupAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    match action {
        GroupAction::AddUser { group, user } => {
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            client.add_user_to_group(user, group).await?;
            write_result(
                &MembershipResult::added(user.as_str(), group.as_str()),
                &format!("Added {user} to group '{group}'"),
                format,
                w.out,
            );
        }
        GroupAction::RemoveUser { group, user } => {
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            client.remove_user_from_group(user, group).await?;
            write_result(
                &MembershipResult::removed(user.as_str(), group.as_str()),
                &format!("Removed {user} from group '{group}'"),
                format,
                w.out,
            );
        }
        GroupAction::ListUsers { group, details } => {
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            let users = client.get_group_members(group, *details).await?;
            let write = if *details {
                write_users_detailed
            } else {
                write_users
            };
            write(&users, format, w.out);
        }
        GroupAction::View { group } => {
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            let info = client.get_group(group).await?;
            write_group_info(&info, format, w.out);
        }
        GroupAction::Create {
            from_json,
            name,
            description,
            is_active,
        } => {
            let params = build_create_params(
                from_json.as_deref(),
                name.as_deref(),
                description.as_deref(),
                *is_active,
            )?;
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
            create_group(&params, ctx, w).await?;
        }
        GroupAction::Update {
            from_json,
            group,
            description,
            is_active,
        } => {
            let (group, params) = build_update_params(
                from_json.as_deref(),
                group.as_deref(),
                description.as_deref(),
                *is_active,
            )?;
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
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            client.update_group(&group, &params).await?;
            write_result(
                &ActionResult::updated_named(group.as_str(), None, ResourceKind::Group),
                &format!("Updated group '{group}'"),
                format,
                w.out,
            );
        }
    }
    Ok(())
}

async fn create_group(
    params: &CreateGroupParams,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    let client = super::runtime::shared::connect_and_configure(ctx).await?;
    let id = client.create_group(params).await?;
    write_result(
        &ActionResult::created_named(id, params.name.as_str(), ResourceKind::Group),
        &format!("Created group #{id} '{}'", params.name),
        format,
        w.out,
    );
    Ok(())
}

#[must_use]
pub fn is_dry_runnable(action: &GroupAction) -> bool {
    matches!(
        action,
        GroupAction::Create { .. } | GroupAction::Update { .. }
    )
}

pub(crate) fn requires_credentials(action: &GroupAction) -> Option<&'static str> {
    match action {
        GroupAction::ListUsers { .. } | GroupAction::View { .. } => None,
        GroupAction::AddUser { .. } => Some("group add-user"),
        GroupAction::RemoveUser { .. } => Some("group remove-user"),
        GroupAction::Create { .. } => Some("group create"),
        GroupAction::Update { .. } => Some("group update"),
    }
}

fn build_create_params(
    from_json: Option<&str>,
    name: Option<&str>,
    description: Option<&str>,
    is_active: Option<bool>,
) -> Result<CreateGroupParams> {
    let mut input = if let Some(arg) = from_json {
        super::runtime::from_json::read_object::<JsonCreateGroup>(arg)?
    } else {
        JsonCreateGroup::default()
    };
    super::runtime::from_json::merge_string(&mut input.name, name);
    super::runtime::from_json::merge_string(&mut input.description, description);
    super::runtime::from_json::merge_copy(&mut input.is_active, is_active);
    Ok(CreateGroupParams {
        name: super::runtime::from_json::required_string(input.name, "name")?,
        description: super::runtime::from_json::required_string(input.description, "description")?,
        is_active: input.is_active.unwrap_or(true),
    })
}

fn build_update_params(
    from_json: Option<&str>,
    group: Option<&str>,
    description: Option<&str>,
    is_active: Option<bool>,
) -> Result<(String, UpdateGroupParams)> {
    let mut input = if let Some(arg) = from_json {
        super::runtime::from_json::read_object::<JsonUpdateGroup>(arg)?
    } else {
        JsonUpdateGroup::default()
    };
    let target = super::runtime::from_json::resolve_string_target(
        group,
        input.group.take(),
        "--from-json object cannot combine positional group with JSON group",
        "--from-json object requires a group",
    )?;
    super::runtime::from_json::merge_string(&mut input.description, description);
    super::runtime::from_json::merge_copy(&mut input.is_active, is_active);
    let params = UpdateGroupParams {
        description: input.description,
        is_active: input.is_active,
    };
    validate_update_params(&params)?;
    Ok((target, params))
}

fn validate_update_params(params: &UpdateGroupParams) -> Result<()> {
    if params.description.is_none() && params.is_active.is_none() {
        return Err(BzrError::InputValidation(
            "no fields to update; specify at least one field to change".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "group_tests.rs"]
mod tests;
