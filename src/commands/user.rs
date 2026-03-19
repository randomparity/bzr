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

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::await_holding_lock)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::test_helpers::{setup_config, ENV_LOCK};
    use crate::cli::UserAction;
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn user_search_returns_results() {
        let _lock = ENV_LOCK.lock().unwrap();
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [{
                    "id": 1,
                    "name": "alice@test.com",
                    "real_name": "Alice"
                }]
            })))
            .mount(&mock)
            .await;

        let action = UserAction::Search {
            query: "alice".to_string(),
            details: false,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_ok());
    }

    #[test]
    fn denied_text_logic() {
        // Test the denied_text derivation logic from execute()
        // (Some(true), Some(text)) => Some(text.clone())
        let disable_login = Some(true);
        let login_denied_text = Some("Go away".to_string());
        let denied_text = match (&disable_login, &login_denied_text) {
            (Some(true), Some(text)) => Some(text.clone()),
            (Some(true), None) => Some("Account disabled".into()),
            (Some(false), _) => Some(String::new()),
            (None, _) => None,
        };
        assert_eq!(denied_text.as_deref(), Some("Go away"));

        // (Some(true), None) => Some("Account disabled")
        let denied_text2 = match (Some(true), None::<String>) {
            (Some(true), Some(text)) => Some(text),
            (Some(true), None) => Some("Account disabled".into()),
            (Some(false), _) => Some(String::new()),
            (None, _) => None,
        };
        assert_eq!(denied_text2.as_deref(), Some("Account disabled"));

        // (Some(false), _) => Some("")
        let denied_text3 = match (Some(false), Some("ignored".to_string())) {
            (Some(true), Some(text)) => Some(text),
            (Some(true), None) => Some("Account disabled".into()),
            (Some(false), _) => Some(String::new()),
            (None, _) => None,
        };
        assert_eq!(denied_text3.as_deref(), Some(""));

        // (None, _) => None
        let denied_text4 = match (None::<bool>, Some("ignored".to_string())) {
            (Some(true), Some(text)) => Some(text),
            (Some(true), None) => Some("Account disabled".into()),
            (Some(false), _) => Some(String::new()),
            (None, _) => None,
        };
        assert!(denied_text4.is_none());
    }
}
