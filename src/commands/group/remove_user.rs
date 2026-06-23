use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::result_types::{write_result, MembershipResult};
use crate::output::writers::Writers;

pub(super) async fn handle(
    group: &str,
    user: &str,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    client.remove_user_from_group(user, group).await?;
    write_result(
        &MembershipResult::removed(user, group),
        &format!("Removed {user} from group '{group}'"),
        ctx.format(),
        w.out,
    );
    Ok(())
}

#[cfg(test)]
#[path = "remove_user_tests.rs"]
mod tests;
