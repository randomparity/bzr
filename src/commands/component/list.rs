use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::component::write_components;
use crate::output::writers::Writers;

pub(super) async fn handle(product: &str, ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let product = client.get_product(product).await?;
    write_components(&product.components, ctx.format(), w.out);
    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
