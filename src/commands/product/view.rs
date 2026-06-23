use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::product::write_product_detail;
use crate::output::writers::Writers;

pub(super) async fn handle(name: &str, ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let product = client.get_product(name).await?;
    write_product_detail(&product, ctx.format(), w.out);
    Ok(())
}
