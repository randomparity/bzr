use crate::cli::ProductAction;
use crate::client::{CreateProductParams, UpdateProductParams};
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn execute(
    action: &ProductAction,
    server: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let client = super::shared::build_client(server).await?;

    match action {
        ProductAction::List { r#type } => {
            let products = client.list_products_by_type(r#type).await?;
            output::print_products(&products, format);
        }
        ProductAction::View { name } => {
            let product = client.get_product(name).await?;
            output::print_product_detail(&product, format);
        }
        ProductAction::Create {
            name,
            description,
            version,
            is_open,
        } => {
            let params = CreateProductParams {
                name: name.clone(),
                description: description.clone(),
                version: version.clone(),
                is_open: *is_open,
            };
            let id = client.create_product(&params).await?;
            #[expect(clippy::print_stdout)]
            {
                println!("Created product #{id} '{name}'");
            }
        }
        ProductAction::Update {
            name,
            description,
            default_milestone,
            is_open,
        } => {
            let params = UpdateProductParams {
                description: description.clone(),
                default_milestone: default_milestone.clone(),
                is_open: *is_open,
            };
            client.update_product(name, &params).await?;
            #[expect(clippy::print_stdout)]
            {
                println!("Updated product '{name}'");
            }
        }
    }
    Ok(())
}
