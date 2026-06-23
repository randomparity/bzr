use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::product::write_products;
use crate::output::writers::Writers;
use crate::types::product::ProductListType;

pub(super) async fn handle(
    list_type: ProductListType,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let products = client.list_products_by_type(list_type).await?;
    write_products(&products, ctx.format(), w.out);
    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
