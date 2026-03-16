use crate::cli::UserAction;
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn execute(
    action: &UserAction,
    server: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let client = super::shared::build_client(server).await?;

    match action {
        UserAction::Search { query } => {
            let users = client.search_users(query).await?;
            output::print_users(&users, format);
        }
        UserAction::Create {
            email,
            full_name,
            password,
        } => {
            let id = client
                .create_user(email, full_name.as_deref(), password.as_deref())
                .await?;
            #[expect(clippy::print_stdout)]
            {
                println!("Created user #{id} ({email})");
            }
        }
        UserAction::Update {
            user,
            real_name,
            email,
            disable_login,
        } => {
            let mut updates = serde_json::Map::new();
            if let Some(n) = real_name {
                updates.insert("real_name".into(), serde_json::Value::String(n.clone()));
            }
            if let Some(e) = email {
                updates.insert("email".into(), serde_json::Value::String(e.clone()));
            }
            if let Some(d) = disable_login {
                updates.insert(
                    "login_denied_text".into(),
                    if *d {
                        serde_json::Value::String("Account disabled".into())
                    } else {
                        serde_json::Value::String(String::new())
                    },
                );
            }
            let body = serde_json::Value::Object(updates);
            client.update_user(user, &body).await?;
            #[expect(clippy::print_stdout)]
            {
                println!("Updated user '{user}'");
            }
        }
    }
    Ok(())
}
