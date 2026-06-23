use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::group::write_group_info;
use crate::output::writers::Writers;

pub(super) async fn handle(group: &str, ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let info = client.get_group(group).await?;
    write_group_info(&info, ctx.format(), w.out);
    Ok(())
}
