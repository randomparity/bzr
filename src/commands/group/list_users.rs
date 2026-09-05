use crate::client::UserDetailLevel;
use crate::commands::runtime::invocation::CommandContext;
use crate::error::Result;
use crate::output::resources::user::{write_users, write_users_detailed};
use crate::output::writers::Writers;

pub(super) async fn handle(
    group: &str,
    details: bool,
    projection_args: &crate::cli::ProjectionArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let projection = crate::validation::fields::projection_for(
        ctx.format(),
        projection_args.fields.as_deref(),
        projection_args.exclude_fields.as_deref(),
        crate::types::user::BUGZILLA_USER_FIELDS,
        w.err,
    )?;
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let detail_level = user_detail_level(details);
    let users = client.get_group_members(group, detail_level).await?;
    let write = match detail_level {
        UserDetailLevel::Basic => write_users,
        UserDetailLevel::Detailed => write_users_detailed,
    };
    write(&users, ctx.format(), &projection, w.table_width(), w.out);
    Ok(())
}

const fn user_detail_level(details: bool) -> UserDetailLevel {
    if details {
        UserDetailLevel::Detailed
    } else {
        UserDetailLevel::Basic
    }
}

#[cfg(test)]
#[path = "list_users_tests.rs"]
mod tests;
