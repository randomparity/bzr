use crate::cli::ProductAction;
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn execute(
    action: &ProductAction,
    server: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let client = super::shared::build_client(server).await?;

    match action {
        ProductAction::List => {
            let products = client.list_products().await?;
            output::print_products(&products, format);
        }
        ProductAction::View { name } => {
            let product = client.get_product(name).await?;
            output::print_product_detail(&product, format);
        }
    }
    Ok(())
}
