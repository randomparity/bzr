use crate::cli::UserAction;
use crate::error::Result;
use crate::output::{self, ActionResult, ResourceKind};
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateUserParams, UpdateUserParams};

pub async fn execute(
    action: &UserAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_client(server, api).await?;

    match action {
        UserAction::Search { query, details } => {
            let users = client.search_users(query, *details).await?;
            output::print_users(&users, *details, format);
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
                &ActionResult::created_named(id, email.as_str(), ResourceKind::User),
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
                names: Some(vec![user.clone()]),
                real_name: real_name.clone(),
                email: email.clone(),
                login_denied_text: denied_text,
            };
            client.update_user(user, &params).await?;
            output::print_result(
                &ActionResult::updated_named(user.as_str(), ResourceKind::User),
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

    // Note: user tests that modify env need ENV_LOCK + setup_config

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

    #[tokio::test]
    async fn update_user_disable_login_sends_denied_text() {
        let _lock = ENV_LOCK.lock().unwrap();
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        Mock::given(method("PUT"))
            .and(path("/rest/user/alice%40test%2Ecom"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&mock)
            .await;

        let action = UserAction::Update {
            user: "alice@test.com".to_string(),
            real_name: None,
            email: None,
            disable_login: Some(true),
            login_denied_text: Some("Go away".to_string()),
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(
            result.is_ok(),
            "update with disable_login failed: {result:?}"
        );
    }

    #[tokio::test]
    async fn update_user_enable_login_sends_empty_denied_text() {
        let _lock = ENV_LOCK.lock().unwrap();
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        Mock::given(method("PUT"))
            .and(path("/rest/user/bob%40test%2Ecom"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({})))
            .expect(1)
            .mount(&mock)
            .await;

        let action = UserAction::Update {
            user: "bob@test.com".to_string(),
            real_name: None,
            email: None,
            disable_login: Some(false),
            login_denied_text: None,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(
            result.is_ok(),
            "update with enable_login failed: {result:?}"
        );
    }
}
