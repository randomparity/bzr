use crate::commands::runtime::invocation::CommandContext;
use crate::error::Result;
use crate::output::resources::group::write_group_info;
use crate::output::writers::Writers;

pub(super) async fn handle(
    group: &str,
    projection_args: &crate::cli::ProjectionArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let projection = crate::validation::fields::projection_for(
        ctx.format(),
        projection_args.fields.as_deref(),
        projection_args.exclude_fields.as_deref(),
        crate::types::group::GROUP_INFO_FIELDS,
        w.err,
    )?;
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let info = client.get_group(group).await?;
    write_group_info(&info, ctx.format(), &projection, w.out);
    Ok(())
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
