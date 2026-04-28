use crate::client::BugzillaClient;
use crate::client::DetectedServerSettings;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::tls::TlsConfig;
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

/// Check if a TLS error should trigger the TOFU (trust-on-first-use) flow.
///
/// Returns `true` when the error is a TLS certificate verification failure
/// and no trust mechanism (insecure, CA cert, or pin) is already configured.
fn should_offer_tofu(err: &BzrError, tls_config: &TlsConfig) -> bool {
    if tls_config.insecure || tls_config.ca_cert_path.is_some() || tls_config.pin_sha256.is_some() {
        return false;
    }
    matches!(err, BzrError::Http(e) if crate::http::is_tls_cert_error(e))
}

/// Check if a reqwest error contains a `PIN_MISMATCH` from the pinned verifier.
fn is_pin_mismatch(err: &BzrError) -> bool {
    matches!(err, BzrError::Http(e) if {
        let chain = crate::error::format_error_chain(e);
        chain.contains("PIN_MISMATCH")
    })
}

/// Check if a reqwest error contains an `ISSUER_CHANGED` from the pinned verifier.
fn is_issuer_changed(err: &BzrError) -> bool {
    matches!(err, BzrError::Http(e) if {
        let chain = crate::error::format_error_chain(e);
        chain.contains("ISSUER_CHANGED")
    })
}

/// Extract the hostname from a URL string, falling back to the raw URL
/// if parsing fails.
fn extract_hostname(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(String::from))
        .unwrap_or_else(|| url.to_string())
}

/// Handle the TOFU flow: probe the server certificate, prompt the user,
/// and if accepted, retry detection and build the client.
async fn handle_tofu(
    server_name: &str,
    url: &str,
    api_key: &str,
    email: Option<&str>,
    api_override: Option<ApiMode>,
    config: &mut Config,
) -> Result<BugzillaClient> {
    let hostname = extract_hostname(url);
    let (fingerprint, issuer) = crate::tls::tofu::probe_server_cert(url).await?;

    let decision = crate::tls::tofu::prompt_tofu(server_name, &hostname, &fingerprint, &issuer)?;

    let tls_config = match decision {
        Some(true) => {
            // "always" — persist pin to config
            if let Some(srv) = config.servers.get_mut(server_name) {
                srv.tls_pin_sha256 = Some(fingerprint.clone());
                srv.tls_pin_issuer = Some(issuer.clone());
                config.save()?;
            }
            TlsConfig {
                pin_sha256: Some(fingerprint),
                pin_issuer: Some(issuer),
                server_name: Some(server_name.to_string()),
                ..Default::default()
            }
        }
        Some(false) => {
            // "y" — trust this specific cert for this session only (no config change)
            TlsConfig {
                pin_sha256: Some(fingerprint),
                pin_issuer: Some(issuer),
                server_name: Some(server_name.to_string()),
                ..Default::default()
            }
        }
        None => {
            return Err(BzrError::config(
                "TLS certificate not trusted. To connect, use one of:\n  \
                 bzr config set-server <NAME> --tls-insecure\n  \
                 bzr config set-server <NAME> --tls-pin-sha256 <PIN>",
            ));
        }
    };

    detect_and_build_client(
        server_name,
        url,
        api_key,
        email,
        api_override,
        &tls_config,
        config,
    )
    .await
}

/// Handle pin mismatch (certificate rotated but issuer unchanged):
/// probe the new cert, prompt the user, and if accepted, update the
/// pin and retry.
async fn handle_pin_rotation(
    server_name: &str,
    url: &str,
    api_key: &str,
    email: Option<&str>,
    api_override: Option<ApiMode>,
    old_pin: &str,
    config: &mut Config,
) -> Result<BugzillaClient> {
    let hostname = extract_hostname(url);
    let (new_fingerprint, issuer) = crate::tls::tofu::probe_server_cert(url).await?;

    let accepted = crate::tls::tofu::prompt_rotation(
        server_name,
        &hostname,
        old_pin,
        &new_fingerprint,
        &issuer,
    )?;

    if !accepted {
        return Err(BzrError::config(format!(
            "certificate rotation rejected for server \"{server_name}\". \
             To clear the pin: bzr config set-server {server_name} \
             --tls-pin-clear"
        )));
    }

    // Update pin in config
    if let Some(srv) = config.servers.get_mut(server_name) {
        srv.tls_pin_sha256 = Some(new_fingerprint.clone());
        srv.tls_pin_issuer = Some(issuer.clone());
        config.save()?;
    }

    let tls_config = TlsConfig {
        pin_sha256: Some(new_fingerprint),
        pin_issuer: Some(issuer),
        server_name: Some(server_name.to_string()),
        ..Default::default()
    };

    detect_and_build_client(
        server_name,
        url,
        api_key,
        email,
        api_override,
        &tls_config,
        config,
    )
    .await
}

/// Detect server settings and build a client, persisting the detected
/// settings to config. Shared tail logic for TOFU and pin rotation flows.
async fn detect_and_build_client(
    server_name: &str,
    url: &str,
    api_key: &str,
    email: Option<&str>,
    api_override: Option<ApiMode>,
    tls_config: &crate::tls::TlsConfig,
    config: &mut Config,
) -> Result<BugzillaClient> {
    let settings = crate::client::detect_server_settings(url, api_key, email, tls_config).await?;
    persist_detected_settings(config, server_name, &settings, true)?;
    let api_mode = api_override.unwrap_or(settings.api_mode);
    BugzillaClient::new(
        url,
        api_key,
        settings.auth_method,
        api_mode,
        email,
        tls_config,
    )
}

/// Run `detect_server_settings` and handle TLS errors with TOFU or
/// pin rotation flows as appropriate.
async fn detect_with_tofu_fallback(
    server_name: &str,
    url: &str,
    api_key: &str,
    email: Option<&str>,
    api_override: Option<ApiMode>,
    tls_config: &TlsConfig,
    config: &mut Config,
) -> Result<DetectOrClient> {
    let result = crate::client::detect_server_settings(url, api_key, email, tls_config).await;

    match result {
        Ok(settings) => Ok(DetectOrClient::Settings(settings)),
        Err(ref e) if should_offer_tofu(e, tls_config) => {
            let client =
                handle_tofu(server_name, url, api_key, email, api_override, config).await?;
            Ok(DetectOrClient::Client(client))
        }
        Err(ref e) if is_pin_mismatch(e) => {
            let old_pin = tls_config.pin_sha256.as_deref().unwrap_or("<unknown>");
            let client = handle_pin_rotation(
                server_name,
                url,
                api_key,
                email,
                api_override,
                old_pin,
                config,
            )
            .await?;
            Ok(DetectOrClient::Client(client))
        }
        Err(ref e) if is_issuer_changed(e) => Err(BzrError::config(format!(
            "TLS certificate issuer changed for server \"{server_name}\" \
                 — this could indicate a MITM attack.\n  \
                 If this is expected, clear the pin and re-connect:\n    \
                 bzr config set-server {server_name} --tls-pin-clear"
        ))),
        Err(e) => Err(e),
    }
}

/// Either detected settings (continue normal flow) or a fully-built
/// client (TOFU/rotation handled everything).
enum DetectOrClient {
    Settings(DetectedServerSettings),
    Client(BugzillaClient),
}

/// Connect to a Bugzilla server with auto-configuration.
///
/// On first connection to a server, detects auth method and API mode, then
/// persists these settings to the config file for subsequent connections.
/// The server's configured email (if any) is stored in the client for
/// Bugzilla 5.0 whoami fallback.
///
/// When a TLS certificate error occurs and no trust mechanism is configured,
/// offers an interactive TOFU (trust-on-first-use) prompt. When a pinned
/// certificate has rotated, offers a rotation prompt.
pub async fn connect_and_configure(
    server: Option<&str>,
    api_override: Option<ApiMode>,
) -> Result<BugzillaClient> {
    let mut config = Config::load()?;
    let (server_name, srv) = config.resolve_server(server)?;
    let tls_config = srv.tls_config(server_name);
    let (server_name, url, api_key, email) = (
        server_name.to_string(),
        srv.url.clone(),
        srv.resolve_api_key(server_name)?,
        srv.email.clone(),
    );

    if tls_config.insecure {
        tracing::warn!("TLS certificate verification disabled for server '{server_name}'");
    }

    // Three cases: fully cached, partially cached (auth only), or uncached.
    let (auth, resolved_mode) = match (srv.auth_method, srv.api_mode) {
        (Some(method), Some(mode)) => (method, mode),
        (Some(method), None) => {
            tracing::debug!("auth_method cached but api_mode missing; re-detecting");
            match detect_with_tofu_fallback(
                &server_name,
                &url,
                &api_key,
                email.as_deref(),
                api_override,
                &tls_config,
                &mut config,
            )
            .await?
            {
                DetectOrClient::Client(client) => return Ok(client),
                DetectOrClient::Settings(settings) => {
                    persist_detected_settings(&mut config, &server_name, &settings, false)?;
                    (method, settings.api_mode)
                }
            }
        }
        _ => {
            match detect_with_tofu_fallback(
                &server_name,
                &url,
                &api_key,
                email.as_deref(),
                api_override,
                &tls_config,
                &mut config,
            )
            .await?
            {
                DetectOrClient::Client(client) => return Ok(client),
                DetectOrClient::Settings(settings) => {
                    persist_detected_settings(&mut config, &server_name, &settings, true)?;
                    (settings.auth_method, settings.api_mode)
                }
            }
        }
    };

    let api_mode = api_override.unwrap_or(resolved_mode);
    let client = BugzillaClient::new(
        &url,
        &api_key,
        auth,
        api_mode,
        email.as_deref(),
        &tls_config,
    )?;
    Ok(client)
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::test_helpers::setup_test_env;
    use crate::ENV_LOCK;

    #[tokio::test]
    async fn connect_client_returns_client() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        // whoami endpoint used by auth detection (already cached in setup_config)
        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
            .mount(&mock)
            .await;

        let result = super::connect_and_configure(None, None).await;
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

        let result = super::connect_and_configure(None, None).await;
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
        let result = super::connect_and_configure(None, Some(crate::types::ApiMode::XmlRpc)).await;
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

        let result = super::connect_and_configure(None, None).await;
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

        let result = super::connect_and_configure(None, None).await;
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

    #[tokio::test]
    async fn connect_client_resolves_env_backed_api_key() {
        let _lock = ENV_LOCK.lock().await;
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();

        let config_dir = tmp.path().join("bzr");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_content = format!(
            r#"
default_server = "test"

[servers.test]
url = "{}"
api_key_env = "BZR_TEST_API_KEY"
auth_method = "header"
api_mode = "rest"
"#,
            mock.uri()
        );
        std::fs::write(config_dir.join("config.toml"), config_content).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            std::fs::set_permissions(&config_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
            std::fs::set_permissions(
                config_dir.join("config.toml"),
                std::fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
        // SAFETY: Tests are serialized via ENV_LOCK; no other threads read these vars concurrently.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", tmp.path());
            std::env::set_var("BZR_TEST_API_KEY", "test-key");
        }

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
            .mount(&mock)
            .await;

        let result = super::connect_and_configure(None, None).await;
        assert!(result.is_ok(), "env-backed config should succeed");
    }
}
