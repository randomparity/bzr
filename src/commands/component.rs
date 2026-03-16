use crate::cli::ComponentAction;
use crate::error::Result;

pub async fn execute(action: &ComponentAction, server: Option<&str>) -> Result<()> {
    let client = super::shared::build_client(server).await?;

    match action {
        ComponentAction::Create {
            product,
            name,
            description,
            default_assignee,
        } => {
            let id = client
                .create_component(product, name, description, default_assignee)
                .await?;
            #[expect(clippy::print_stdout)]
            {
                println!("Created component #{id} in product '{product}'");
            }
        }
        ComponentAction::Update {
            id,
            name,
            description,
            default_assignee,
        } => {
            let mut updates = serde_json::Map::new();
            if let Some(n) = name {
                updates.insert("name".into(), serde_json::Value::String(n.clone()));
            }
            if let Some(d) = description {
                updates.insert("description".into(), serde_json::Value::String(d.clone()));
            }
            if let Some(a) = default_assignee {
                updates.insert(
                    "default_assignee".into(),
                    serde_json::Value::String(a.clone()),
                );
            }
            let body = serde_json::Value::Object(updates);
            client.update_component(*id, &body).await?;
            #[expect(clippy::print_stdout)]
            {
                println!("Updated component #{id}");
            }
        }
    }
    Ok(())
}
