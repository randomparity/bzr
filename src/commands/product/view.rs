use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::product::write_product_detail;
use crate::output::writers::Writers;

pub(super) async fn handle(
    name: &str,
    projection_args: &crate::cli::ProjectionArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let projection = crate::validation::fields::projection_for(
        ctx.format(),
        projection_args.fields.as_deref(),
        projection_args.exclude_fields.as_deref(),
        crate::types::product::PRODUCT_FIELDS,
        w.err,
    )?;
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let product = client.get_product(name).await?;
    write_product_detail(&product, ctx.format(), &projection, w.out);
    Ok(())
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
