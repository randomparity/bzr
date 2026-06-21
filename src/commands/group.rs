use crate::cli::GroupAction;
use crate::error::{BzrError, Result};
use crate::output::resources::group::write_group_info;
use crate::output::resources::user::{write_users, write_users_detailed};
use crate::output::result_types::{
    write_result, ActionResult, DryRunResult, MembershipResult, ResourceKind,
};
use crate::output::writers::Writers;
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateGroupParams, UpdateGroupParams};

pub async fn execute(
    action: &GroupAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    validate_action(action)?;
    let client = super::runtime::shared::connect_and_configure(server, api).await?;

    match action {
        GroupAction::AddUser { group, user } => {
            client.add_user_to_group(user, group).await?;
            write_result(
                &MembershipResult::added(user.as_str(), group.as_str()),
                &format!("Added {user} to group '{group}'"),
                format,
                w.out,
            );
        }
        GroupAction::RemoveUser { group, user } => {
            client.remove_user_from_group(user, group).await?;
            write_result(
                &MembershipResult::removed(user.as_str(), group.as_str()),
                &format!("Removed {user} from group '{group}'"),
                format,
                w.out,
            );
        }
        GroupAction::ListUsers { group, details } => {
            let users = client.get_group_members(group, *details).await?;
            if *details {
                write_users_detailed(&users, format, w.out);
            } else {
                write_users(&users, format, w.out);
            }
        }
        GroupAction::View { group } => {
            let info = client.get_group(group).await?;
            write_group_info(&info, format, w.out);
        }
        GroupAction::Create {
            name,
            description,
            is_active,
        } => {
            let params = CreateGroupParams {
                name: name.clone(),
                description: description.clone(),
                is_active: *is_active,
            };
            if super::runtime::dry_run::enabled() {
                let message = format!("Would create group '{name}'");
                write_result(
                    &DryRunResult::new(ResourceKind::Group, &[], &params),
                    &message,
                    format,
                    w.out,
                );
                return Ok(());
            }
            let id = client.create_group(&params).await?;
            write_result(
                &ActionResult::created_named(id, name.as_str(), ResourceKind::Group),
                &format!("Created group #{id} '{name}'"),
                format,
                w.out,
            );
        }
        GroupAction::Update {
            group,
            description,
            is_active,
        } => {
            let params = UpdateGroupParams {
                description: description.clone(),
                is_active: *is_active,
            };
            if super::runtime::dry_run::enabled() {
                let message = format!("Would update group '{group}'");
                write_result(
                    &DryRunResult::new(ResourceKind::Group, &[], &params),
                    &message,
                    format,
                    w.out,
                );
                return Ok(());
            }
            client.update_group(group, &params).await?;
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

#[must_use]
pub fn is_dry_runnable(action: &GroupAction) -> bool {
    matches!(
        action,
        GroupAction::Create { .. } | GroupAction::Update { .. }
    )
}

fn validate_action(action: &GroupAction) -> Result<()> {
    if let GroupAction::Update {
        description,
        is_active,
        ..
    } = action
    {
        if description.is_none() && is_active.is_none() {
            return Err(BzrError::InputValidation(
                "no fields to update; specify at least one field to change".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "group_tests.rs"]
mod tests;
