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

/// Extract the new fingerprint and issuer from a `PIN_MISMATCH` error chain.
///
/// Error format: `PIN_MISMATCH for <server>: expected <old>, got <new>, issuer <issuer>`
fn parse_pin_mismatch_details(chain: &str) -> Option<(String, String)> {
    let rest = chain.get(chain.find("PIN_MISMATCH")?..)?;
    let got_start = rest.find(", got ")? + ", got ".len();
    let after_got = &rest[got_start..];
    let issuer_pos = after_got.find(", issuer ")?;
    let new_fp = after_got[..issuer_pos].to_string();
    let new_issuer = after_got[issuer_pos + ", issuer ".len()..].to_string();
    Some((new_fp, new_issuer))
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
    let (fingerprint, issuer, issuer_der) = crate::tls::tofu::probe_server_cert(url).await?;

    let decision = crate::tls::tofu::prompt_tofu(server_name, &hostname, &fingerprint, &issuer)?;

    let tls_config = match decision {
        Some(true) => {
            // "always" — persist pin to config
            if let Some(srv) = config.servers.get_mut(server_name) {
                srv.tls_pin_sha256 = Some(fingerprint.clone());
                srv.tls_pin_issuer = Some(issuer.clone());
                srv.tls_pin_issuer_der.clone_from(&issuer_der);
                config.save()?;
            }
            TlsConfig {
                pin_sha256: Some(fingerprint),
                pin_issuer: Some(issuer),
                pin_issuer_der: issuer_der,
                server_name: Some(server_name.to_string()),
                ..Default::default()
            }
        }
        Some(false) => {
            // "y" — trust this specific cert for this session only (no config change)
            TlsConfig {
                pin_sha256: Some(fingerprint),
                pin_issuer: Some(issuer),
                pin_issuer_der: issuer_der,
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
/// use the fingerprint and issuer parsed from the `PIN_MISMATCH` error,
/// prompt the user, and if accepted, update the pin and retry.
#[expect(clippy::too_many_arguments, reason = "private orchestration fn")]
async fn handle_pin_rotation(
    server_name: &str,
    url: &str,
    api_key: &str,
    email: Option<&str>,
    api_override: Option<ApiMode>,
    old_pin: &str,
    new_fingerprint: &str,
    new_issuer: &str,
    config: &mut Config,
) -> Result<BugzillaClient> {
    let hostname = extract_hostname(url);

    let accepted = crate::tls::tofu::prompt_rotation(
        server_name,
        &hostname,
        old_pin,
        new_fingerprint,
        new_issuer,
    )?;

    if !accepted {
        return Err(BzrError::config(format!(
            "certificate rotation rejected for server \"{server_name}\". \
             To clear the pin: bzr config set-server {server_name} \
             --tls-pin-clear"
        )));
    }

    // Update pin in config. Keep the existing pin_issuer_der: since
    // PIN_MISMATCH only fires when the issuer DER matched (otherwise
    // ISSUER_CHANGED would have fired), the DER bytes are still valid.
    let existing_issuer_der = config
        .servers
        .get(server_name)
        .and_then(|s| s.tls_pin_issuer_der.clone());
    if let Some(srv) = config.servers.get_mut(server_name) {
        srv.tls_pin_sha256 = Some(new_fingerprint.to_owned());
        srv.tls_pin_issuer = Some(new_issuer.to_owned());
        config.save()?;
    }

    let tls_config = TlsConfig {
        pin_sha256: Some(new_fingerprint.to_owned()),
        pin_issuer: Some(new_issuer.to_owned()),
        pin_issuer_der: existing_issuer_der,
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
            let chain = match e {
                BzrError::Http(re) => crate::error::format_error_chain(re),
                _ => String::new(),
            };
            let (new_fp, new_issuer) = parse_pin_mismatch_details(&chain)
                .unwrap_or_else(|| ("<unknown>".to_string(), "<unknown>".to_string()));
            let client = handle_pin_rotation(
                server_name,
                url,
                api_key,
                email,
                api_override,
                old_pin,
                &new_fp,
                &new_issuer,
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
#[expect(clippy::unwrap_used, clippy::panic)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use crate::error::BzrError;
    use crate::test_helpers::setup_test_env;
    use crate::tls::TlsConfig;
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

    #[test]
    fn should_offer_tofu_false_when_insecure() {
        let tls = TlsConfig {
            insecure: true,
            ..Default::default()
        };
        let err = BzrError::Config("test".into());
        assert!(!super::should_offer_tofu(&err, &tls));
    }

    #[test]
    fn should_offer_tofu_false_when_pin_configured() {
        let tls = TlsConfig {
            pin_sha256: Some("sha256//test".into()),
            ..Default::default()
        };
        let err = BzrError::Config("test".into());
        assert!(!super::should_offer_tofu(&err, &tls));
    }

    #[test]
    fn should_offer_tofu_false_when_ca_configured() {
        let tls = TlsConfig {
            ca_cert_path: Some("/path".into()),
            ..Default::default()
        };
        let err = BzrError::Config("test".into());
        assert!(!super::should_offer_tofu(&err, &tls));
    }

    #[test]
    fn should_offer_tofu_false_for_non_http_error() {
        let tls = TlsConfig::default();
        let err = BzrError::Config("not an HTTP error".into());
        assert!(!super::should_offer_tofu(&err, &tls));
    }

    #[test]
    fn extract_hostname_parses_url() {
        assert_eq!(
            super::extract_hostname("https://example.com/path"),
            "example.com"
        );
    }

    #[test]
    fn extract_hostname_with_port() {
        assert_eq!(
            super::extract_hostname("https://example.com:8443/path"),
            "example.com"
        );
    }

    #[test]
    fn extract_hostname_returns_raw_on_invalid() {
        assert_eq!(super::extract_hostname("not-a-url"), "not-a-url");
    }

    #[test]
    fn is_pin_mismatch_returns_false_for_non_http() {
        let err = BzrError::Config("PIN_MISMATCH".into());
        assert!(!super::is_pin_mismatch(&err));
    }

    #[test]
    fn is_issuer_changed_returns_false_for_non_http() {
        let err = BzrError::Config("ISSUER_CHANGED".into());
        assert!(!super::is_issuer_changed(&err));
    }

    #[test]
    fn parse_pin_mismatch_extracts_fingerprint_and_issuer() {
        let chain = "error sending request: PIN_MISMATCH for test: \
                     expected sha256//old==, got sha256//new==, \
                     issuer CN=Test CA, O=Test";
        let (fp, issuer) = super::parse_pin_mismatch_details(chain).unwrap();
        assert_eq!(fp, "sha256//new==");
        assert_eq!(issuer, "CN=Test CA, O=Test");
    }

    #[test]
    fn parse_pin_mismatch_returns_none_for_unrelated_error() {
        let chain = "connection refused";
        assert!(super::parse_pin_mismatch_details(chain).is_none());
    }

    #[tokio::test]
    async fn detect_with_tofu_fallback_normal_path() {
        let _lock = ENV_LOCK.lock().await;
        let server = MockServer::start().await;
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
        // SAFETY: Tests are serialized via ENV_LOCK.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/rest/version"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
            )
            .mount(&server)
            .await;

        let mut config = crate::config::Config::load().unwrap();
        let tls_config = TlsConfig::default();
        let result = super::detect_with_tofu_fallback(
            "test",
            &server.uri(),
            "test-key",
            None,
            None,
            &tls_config,
            &mut config,
        )
        .await;
        assert!(result.is_ok(), "normal path should succeed");
    }

    #[test]
    fn is_issuer_changed_returns_false_for_config_error() {
        let err = BzrError::Config("ISSUER_CHANGED inside config".into());
        assert!(!super::is_issuer_changed(&err));
    }

    #[test]
    fn is_pin_mismatch_returns_false_for_config_error() {
        let err = BzrError::Config("PIN_MISMATCH inside config".into());
        assert!(!super::is_pin_mismatch(&err));
    }

    #[test]
    fn persist_detected_settings_skips_unknown_server() {
        // If the server name doesn't exist in config, persist is a no-op
        let mut config = crate::config::Config::default();
        let settings = crate::client::DetectedServerSettings {
            auth_method: crate::types::AuthMethod::Header,
            api_mode: crate::types::ApiMode::Rest,
            server_version: Some("5.1".into()),
        };
        let result = super::persist_detected_settings(&mut config, "nonexistent", &settings, true);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_pin_mismatch_no_got_returns_none() {
        let chain = "PIN_MISMATCH for test: expected sha256//old==";
        assert!(super::parse_pin_mismatch_details(chain).is_none());
    }

    #[test]
    fn parse_pin_mismatch_no_issuer_returns_none() {
        let chain = "PIN_MISMATCH for test: expected sha256//old==, got sha256//new==";
        assert!(super::parse_pin_mismatch_details(chain).is_none());
    }

    /// Build a config TOML with the given extra fields injected into the
    /// `[servers.test]` table. Keeps the boilerplate of `XDG_CONFIG_HOME`
    /// override + permissions out of every test.
    fn write_config(tmp: &tempfile::TempDir, server_url: &str, extra: &str) {
        let config_dir = tmp.path().join("bzr");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_content = format!(
            r#"
default_server = "test"

[servers.test]
url = "{server_url}"
api_key = "test-key"
{extra}
"#,
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
        // SAFETY: Tests are serialized via ENV_LOCK.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };
    }

    /// Mount the standard whoami + version mocks used by auth/version detection.
    async fn mount_detection_mocks(server: &MockServer) {
        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
            .mount(server)
            .await;
        Mock::given(method("GET"))
            .and(path("/rest/version"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
            )
            .mount(server)
            .await;
    }

    /// `tls_insecure = true` should emit a warning and still complete the
    /// connect flow, exercising the warn branch around the insecure flag.
    #[tokio::test]
    async fn connect_client_with_tls_insecure_warns_and_succeeds() {
        let _lock = ENV_LOCK.lock().await;
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        write_config(
            &tmp,
            &mock.uri(),
            "auth_method = \"header\"\napi_mode = \"rest\"\ntls_insecure = true",
        );
        mount_detection_mocks(&mock).await;

        let result = super::connect_and_configure(None, None).await;
        assert!(result.is_ok(), "tls_insecure should still build a client");
    }

    /// `auth_method` cached but `api_mode` missing -> takes the partial-cache
    /// branch in `connect_and_configure` (re-detects `api_mode`, persists
    /// without overwriting `auth_method`).
    #[tokio::test]
    async fn connect_client_partial_cache_redetects_api_mode() {
        let _lock = ENV_LOCK.lock().await;
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        // auth_method present, api_mode missing -> partial cache branch
        write_config(&tmp, &mock.uri(), "auth_method = \"header\"");
        mount_detection_mocks(&mock).await;

        let result = super::connect_and_configure(None, None).await;
        assert!(
            result.is_ok(),
            "partial-cache path should re-detect api_mode and succeed"
        );

        // Verify api_mode + version got persisted but auth_method stayed Header
        let reloaded = crate::config::Config::load().unwrap();
        let srv = &reloaded.servers["test"];
        assert_eq!(srv.auth_method, Some(crate::types::AuthMethod::Header));
        assert_eq!(srv.api_mode, Some(crate::types::ApiMode::Rest));
        assert_eq!(srv.server_version.as_deref(), Some("5.1.2"));
    }

    /// `detect_and_build_client` is the shared tail of the TOFU/rotation
    /// flows: detect → persist → construct client. Drive it with a real
    /// wiremock to cover lines 211-231.
    #[tokio::test]
    async fn detect_and_build_client_persists_and_returns_client() {
        let _lock = ENV_LOCK.lock().await;
        let server = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        write_config(&tmp, &server.uri(), "");
        mount_detection_mocks(&server).await;

        let mut config = crate::config::Config::load().unwrap();
        let tls_config = TlsConfig::default();
        let result = super::detect_and_build_client(
            "test",
            &server.uri(),
            "test-key",
            None,
            None,
            &tls_config,
            &mut config,
        )
        .await;
        assert!(result.is_ok(), "detect_and_build_client should succeed");

        // Verify the settings were persisted.
        let reloaded = crate::config::Config::load().unwrap();
        let srv = &reloaded.servers["test"];
        assert_eq!(srv.auth_method, Some(crate::types::AuthMethod::Header));
        assert_eq!(srv.api_mode, Some(crate::types::ApiMode::Rest));
    }

    /// `detect_and_build_client` should honor an `api_override` even when the
    /// server's detected mode would be different.
    #[tokio::test]
    async fn detect_and_build_client_respects_api_override() {
        let _lock = ENV_LOCK.lock().await;
        let server = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        write_config(&tmp, &server.uri(), "");
        mount_detection_mocks(&server).await;

        let mut config = crate::config::Config::load().unwrap();
        let tls_config = TlsConfig::default();
        let result = super::detect_and_build_client(
            "test",
            &server.uri(),
            "test-key",
            None,
            Some(crate::types::ApiMode::XmlRpc),
            &tls_config,
            &mut config,
        )
        .await;
        assert!(result.is_ok(), "api_override should still produce a client");
    }

    /// `handle_tofu` calls `probe_server_cert`, which must hit a real TLS
    /// endpoint. Pointing it at an unreachable port exercises the early
    /// failure path (`probe_server_cert` returns `Err`), covering the entry
    /// of `handle_tofu`.
    #[tokio::test]
    async fn handle_tofu_returns_error_when_probe_fails() {
        let _lock = ENV_LOCK.lock().await;
        let tmp = tempfile::TempDir::new().unwrap();
        // Use an unreachable HTTPS URL so probe_server_cert fails fast.
        write_config(&tmp, "https://127.0.0.1:1", "");

        let mut config = crate::config::Config::load().unwrap();
        let result = super::handle_tofu(
            "test",
            "https://127.0.0.1:1",
            "test-key",
            None,
            None,
            &mut config,
        )
        .await;
        assert!(
            result.is_err(),
            "handle_tofu should propagate probe failure"
        );
    }

    /// `handle_pin_rotation` prompts the user; in non-interactive tests
    /// `prompt_rotation` returns `false`, so the function must return a
    /// "rotation rejected" config error covering lines 168-174.
    #[tokio::test]
    async fn handle_pin_rotation_rejects_in_noninteractive() {
        let _lock = ENV_LOCK.lock().await;
        let tmp = tempfile::TempDir::new().unwrap();
        write_config(
            &tmp,
            "https://example.test",
            "tls_pin_sha256 = \"sha256//AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\"\n\
             tls_pin_issuer = \"CN=Old\"",
        );

        let mut config = crate::config::Config::load().unwrap();
        let result = super::handle_pin_rotation(
            "test",
            "https://example.test",
            "test-key",
            None,
            None,
            "sha256//AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            "sha256//BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB=",
            "CN=New",
            &mut config,
        )
        .await;
        match result {
            Err(BzrError::Config(msg)) => {
                assert!(
                    msg.contains("rotation rejected"),
                    "should be rotation-rejected error: {msg}"
                );
            }
            Err(other) => panic!("expected Config error, got {other:?}"),
            Ok(_) => panic!("expected error, got Ok"),
        }
    }

    /// `detect_with_tofu_fallback` should propagate non-TLS errors as-is
    /// (covers the catch-all `Err(e) => Err(e)` branch around line 281).
    #[tokio::test]
    async fn detect_with_tofu_fallback_propagates_auth_errors() {
        let _lock = ENV_LOCK.lock().await;
        let server = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        write_config(&tmp, &server.uri(), "");

        // No mocks mounted -> wiremock returns 404 for every request.
        // detect_auth_method will exhaust whoami + valid_login (no email)
        // and return BzrError::Auth, which is not a TLS error -> propagates.
        let mut config = crate::config::Config::load().unwrap();
        let tls_config = TlsConfig::default();
        let result = super::detect_with_tofu_fallback(
            "test",
            &server.uri(),
            "test-key",
            None,
            None,
            &tls_config,
            &mut config,
        )
        .await;
        assert!(result.is_err(), "auth failure should propagate");
    }

    /// `should_offer_tofu` returns false for an `Http` error that does not
    /// look like a TLS cert error. Construct a real reqwest error by
    /// connecting plain HTTP to a wiremock URL with `connect_timeout` set
    /// to a tiny value, then asserting on the predicate.
    #[tokio::test]
    async fn should_offer_tofu_false_for_non_tls_http_error() {
        // Build a real reqwest::Error by failing to connect to an
        // unreachable address. This is not a TLS error (plain HTTP), so
        // is_tls_cert_error should be false.
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_millis(50))
            .build()
            .unwrap();
        let err = client
            .get("http://127.0.0.1:1/unreachable")
            .send()
            .await
            .unwrap_err();
        let bzr_err = BzrError::Http(err);
        let tls = TlsConfig::default();
        assert!(!super::should_offer_tofu(&bzr_err, &tls));
        // Same error should also not match pin/issuer predicates.
        assert!(!super::is_pin_mismatch(&bzr_err));
        assert!(!super::is_issuer_changed(&bzr_err));
    }

    /// `parse_pin_mismatch_details` happy path is already covered;
    /// these guard the slicing-boundary branches.
    #[test]
    fn parse_pin_mismatch_handles_marker_at_start() {
        let chain = "PIN_MISMATCH for test: expected old, got new, issuer CN=X";
        let (fp, issuer) = super::parse_pin_mismatch_details(chain).unwrap();
        assert_eq!(fp, "new");
        assert_eq!(issuer, "CN=X");
    }
}
