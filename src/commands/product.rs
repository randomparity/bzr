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
            let body = serde_json::json!({
                "name": name,
                "description": description,
                "version": version,
                "is_open": is_open,
            });
            let id = client.create_product(&body).await?;
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
            let mut updates = serde_json::Map::new();
            if let Some(d) = description {
                updates.insert("description".into(), serde_json::Value::String(d.clone()));
            }
            if let Some(m) = default_milestone {
                updates.insert(
                    "default_milestone".into(),
                    serde_json::Value::String(m.clone()),
                );
            }
            if let Some(o) = is_open {
                updates.insert("is_open".into(), serde_json::Value::Bool(*o));
            }
            let body = serde_json::Value::Object(updates);
            client.update_product(name, &body).await?;
            #[expect(clippy::print_stdout)]
            {
                println!("Updated product '{name}'");
            }
        }
    }
    Ok(())
}
