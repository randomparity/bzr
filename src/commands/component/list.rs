use crate::commands::runtime::invocation::CommandContext;
use crate::error::Result;
use crate::output::resources::component::write_components;
use crate::output::writers::Writers;

pub(super) async fn handle(
    product: &str,
    projection_args: &crate::cli::ProjectionArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let projection = crate::validation::fields::projection_for(
        ctx.format(),
        projection_args.fields.as_deref(),
        projection_args.exclude_fields.as_deref(),
        crate::types::component::COMPONENT_FIELDS,
        w.err,
    )?;
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let product = client.get_product(product).await?;
    write_components(
        &product.components,
        ctx.format(),
        &projection,
        w.table_width(),
        w.out,
    );
    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
