use crate::commands::runtime::invocation::CommandContext;
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
    client.add_user_to_group(user, group).await?;
    write_result(
        &MembershipResult::added(user, group),
        &format!("Added {user} to group '{group}'"),
        ctx.format(),
        w.out,
    );
    Ok(())
}

#[cfg(test)]
#[path = "add_user_tests.rs"]
mod tests;
