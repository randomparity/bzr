use crate::client::UserDetailLevel;
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::user::{write_users, write_users_detailed};
use crate::output::writers::Writers;

pub(super) async fn handle(
    group: &str,
    details: bool,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let detail_level = user_detail_level(details);
    let users = client.get_group_members(group, detail_level).await?;
    let write = match detail_level {
        UserDetailLevel::Basic => write_users,
        UserDetailLevel::Detailed => write_users_detailed,
    };
    write(&users, ctx.format(), w.out);
    Ok(())
}

const fn user_detail_level(details: bool) -> UserDetailLevel {
    if details {
        UserDetailLevel::Detailed
    } else {
        UserDetailLevel::Basic
    }
}
