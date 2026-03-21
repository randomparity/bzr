use crate::client::BugzillaClient;
use crate::config::Config;
use crate::error::Result;
use crate::types::ApiMode;

/// Like [`connect_client_with_email`], but discards the server email.
/// Used by all commands that do not need the Bugzilla 5.0 `valid_login` fallback.
pub async fn connect_client(
    server: Option<&str>,
    api_override: Option<ApiMode>,
) -> Result<BugzillaClient> {
    let (client, _email) = connect_client_with_email(server, api_override).await?;
    Ok(client)
}

/// Connect to a Bugzilla server, also returning the server's configured email
/// (if any). The email is needed by whoami for Bugzilla 5.0 fallback.
pub async fn connect_client_with_email(
    server: Option<&str>,
    api_override: Option<ApiMode>,
) -> Result<(BugzillaClient, Option<String>)> {
    let mut config = Config::load()?;
    let (server_name, srv) = config.resolve_server(server)?;
    let (server_name, url, api_key, email) = (
        server_name.to_string(),
        srv.url.clone(),
        srv.api_key.clone(),
        srv.email.clone(),
    );
    let (auth, detected_mode) =
        crate::client::auth::detect_and_persist_server_settings(&mut config, &server_name).await?;
    let api_mode = api_override.unwrap_or(detected_mode);
    let client = BugzillaClient::new(&url, &api_key, auth, api_mode)?;
    Ok((client, email))
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::test_helpers::{setup_config, ENV_LOCK};

    #[tokio::test]
    async fn connect_client_returns_client() {
        let _lock = ENV_LOCK.lock().await;
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        // whoami endpoint used by auth detection (already cached in setup_config)
        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
            .mount(&mock)
            .await;

        let result = super::connect_client(None, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn connect_client_with_email_returns_email() {
        let _lock = ENV_LOCK.lock().await;
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();

        // Set up config with an email field
        let config_dir = tmp.path().join("bzr");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_content = format!(
            r#"
default_server = "test"

[servers.test]
url = "{}"
api_key = "test-key"
auth_method = "header"
api_mode = "rest"
email = "user@example.com"
"#,
            mock.uri()
        );
        std::fs::write(config_dir.join("config.toml"), config_content).unwrap();
        // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
            .mount(&mock)
            .await;

        let (_, email) = super::connect_client_with_email(None, None).await.unwrap();
        assert_eq!(email.as_deref(), Some("user@example.com"));
    }

    #[tokio::test]
    async fn connect_client_api_override_applies() {
        let _lock = ENV_LOCK.lock().await;
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
            .mount(&mock)
            .await;

        // Override with XmlRpc mode — connect should still succeed
        let result = super::connect_client(None, Some(crate::types::ApiMode::XmlRpc)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn connect_client_missing_server_fails() {
        let _lock = ENV_LOCK.lock().await;
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join("bzr");
        std::fs::create_dir_all(&config_dir).unwrap();
        // Config with no servers
        std::fs::write(config_dir.join("config.toml"), "").unwrap();
        // SAFETY: Tests are serialized via ENV_LOCK.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };

        let result = super::connect_client(None, None).await;
        assert!(result.is_err());
    }
}
