use crate::client::auth::DetectedServerSettings;
use crate::client::BugzillaClient;
use crate::config::Config;
use crate::error::Result;
use crate::types::ApiMode;

/// Persist detected server settings to config.
/// Always persists `auth_method` when `persist_auth` is true.
/// Only persists `api_mode` and `server_version` when version detection
/// succeeded (`server_version` is `Some`).
fn persist_detected_settings(
    config: &mut Config,
    server_name: &str,
    settings: &DetectedServerSettings,
    persist_auth: bool,
) -> Result<()> {
    if let Some(srv_mut) = config.servers.get_mut(server_name) {
        if persist_auth {
            srv_mut.auth_method = Some(settings.auth_method);
        }
        if settings.server_version.is_some() {
            srv_mut.api_mode = Some(settings.api_mode);
            srv_mut.server_version.clone_from(&settings.server_version);
        }
        config.save()?;
    }
    Ok(())
}

/// Connect to a Bugzilla server.
///
/// On first connection to a server, detects auth method and API mode, then
/// persists these settings to the config file for subsequent connections.
/// The server's configured email (if any) is stored in the client for
/// Bugzilla 5.0 whoami fallback.
pub async fn connect_client(
    server: Option<&str>,
    api_override: Option<ApiMode>,
) -> Result<BugzillaClient> {
    let mut config = Config::load()?;
    let (server_name, srv) = config.resolve_server(server)?;
    let (server_name, url, api_key, email) = (
        server_name.to_string(),
        srv.url.clone(),
        srv.api_key.clone(),
        srv.email.clone(),
    );
    // Use cached auth method if available; otherwise detect via network probes
    // and persist the results back to config.
    let (auth, resolved_mode) = if let Some(method) = srv.auth_method {
        let mode = if let Some(m) = srv.api_mode {
            m
        } else {
            tracing::debug!("auth_method cached but api_mode missing; re-detecting api_mode");
            let settings =
                crate::client::detect_server_settings(&url, &api_key, email.as_deref()).await?;
            persist_detected_settings(&mut config, &server_name, &settings, false)?;
            settings.api_mode
        };
        (method, mode)
    } else {
        let settings =
            crate::client::detect_server_settings(&url, &api_key, email.as_deref()).await?;
        persist_detected_settings(&mut config, &server_name, &settings, true)?;
        (settings.auth_method, settings.api_mode)
    };

    let api_mode = api_override.unwrap_or(resolved_mode);
    let client = BugzillaClient::new(&url, &api_key, auth, api_mode, email.as_deref())?;
    Ok(client)
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::test_helpers::{setup_test_env, ENV_LOCK};

    #[tokio::test]
    async fn connect_client_returns_client() {
        let (_lock, mock, _tmp) = setup_test_env().await;

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
    async fn connect_client_with_email_config_succeeds() {
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

        let result = super::connect_client(None, None).await;
        assert!(
            result.is_ok(),
            "connect_client with email config should succeed"
        );
    }

    #[tokio::test]
    async fn connect_client_api_override_applies() {
        let (_lock, mock, _tmp) = setup_test_env().await;

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

    /// Exercises the full orchestration: no cached auth -> probes server -> persists result.
    #[tokio::test]
    async fn uncached_auth_detects_and_persists() {
        let _lock = ENV_LOCK.lock().await;
        let server = MockServer::start().await;

        // whoami succeeds with header auth -> detects Header auth method
        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
            .mount(&server)
            .await;

        // version endpoint -> detects REST mode
        Mock::given(method("GET"))
            .and(path("/rest/version"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
            )
            .mount(&server)
            .await;

        // Set up a real config file so config.save() works
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join("bzr");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_content = format!(
            r#"
default_server = "test"

[servers.test]
url = "{}"
api_key = "test-key"
"#,
            server.uri()
        );
        std::fs::write(config_dir.join("config.toml"), &config_content).unwrap();
        // SAFETY: Tests are serialized via ENV_LOCK.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };

        let result = super::connect_client(None, None).await;
        assert!(result.is_ok(), "connect_client should succeed");

        // Verify persistence: reload from disk
        let reloaded = crate::config::Config::load().unwrap();
        assert_eq!(
            reloaded.servers["test"].auth_method,
            Some(crate::types::AuthMethod::Header)
        );
        assert_eq!(
            reloaded.servers["test"].api_mode,
            Some(crate::types::ApiMode::Rest)
        );
        assert_eq!(
            reloaded.servers["test"].server_version.as_deref(),
            Some("5.1.2")
        );
    }
}
