use crate::cli::UserAction;
use crate::config::ApiMode;
use crate::error::Result;
use crate::output;
use crate::types::OutputFormat;
use crate::types::{CreateUserParams, UpdateUserParams};

pub async fn execute(
    action: &UserAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::build_client(server, api).await?;

    match action {
        UserAction::Search { query, details } => {
            let users = client.search_users(query, *details).await?;
            output::print_users(&users, format, *details);
        }
        UserAction::Create {
            email,
            full_name,
            password,
        } => {
            let params = CreateUserParams {
                email: email.clone(),
                full_name: full_name.clone(),
                password: password.clone(),
            };
            let id = client.create_user(&params).await?;
            output::print_result(
                &serde_json::json!({"id": id, "email": email, "resource": "user", "action": "created"}),
                &format!("Created user #{id} ({email})"),
                format,
            );
        }
        UserAction::Update {
            user,
            real_name,
            email,
            disable_login,
            login_denied_text,
        } => {
            let denied_text = match (disable_login, login_denied_text) {
                (Some(true), Some(text)) => Some(text.clone()),
                (Some(true), None) => Some("Account disabled".into()),
                (Some(false), _) => Some(String::new()),
                (None, _) => None,
            };
            let params = UpdateUserParams {
                real_name: real_name.clone(),
                email: email.clone(),
                login_denied_text: denied_text,
            };
            client.update_user(user, &params).await?;
            output::print_result(
                &serde_json::json!({"user": user, "resource": "user", "action": "updated"}),
                &format!("Updated user '{user}'"),
                format,
            );
        }
    }
    Ok(())
}
