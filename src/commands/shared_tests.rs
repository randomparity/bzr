#![expect(clippy::unwrap_used, clippy::panic)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::error::BzrError;
use crate::test_helpers::setup_test_env;
use crate::tls::TlsConfig;
use crate::ENV_LOCK;

fn connect_context(
    server_name: &str,
    url: &str,
    api_override: Option<crate::types::ApiMode>,
) -> super::ConnectContext {
    super::ConnectContext {
        server_name: server_name.to_string(),
        url: url.to_string(),
        api_key: "test-key".to_string(),
        email: None,
        api_override,
    }
}

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
    let ctx = connect_context("test", &server.uri(), None);
    let result = super::detect_with_tofu_fallback(&ctx, &tls_config, &mut config).await;
    assert!(result.is_ok(), "normal path should succeed");
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

/// The partial-cache branch must preserve the cached `auth_method` even
/// when re-detection would have picked a different method. Re-detection
/// runs to fill in the missing `api_mode` only.
#[tokio::test]
async fn connect_client_partial_cache_preserves_cached_auth_method() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    // Cache says "header"; live detection will reject header (401) and
    // accept query_param (200). The cached "header" must survive.
    write_config(&tmp, &mock.uri(), "auth_method = \"header\"");

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .and(wiremock::matchers::header("X-BUGZILLA-API-KEY", "test-key"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .and(wiremock::matchers::query_param(
            "Bugzilla_api_key",
            "test-key",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .mount(&mock)
        .await;

    let result = super::connect_and_configure(None, None).await;
    assert!(
        result.is_ok(),
        "partial-cache with disagreeing detection should succeed: {:?}",
        result.err()
    );

    let reloaded = crate::config::Config::load().unwrap();
    let srv = &reloaded.servers["test"];
    assert_eq!(
        srv.auth_method,
        Some(crate::types::AuthMethod::Header),
        "cached auth_method must not be overwritten by re-detection"
    );
    assert_eq!(srv.api_mode, Some(crate::types::ApiMode::Rest));
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
    let ctx = connect_context("test", &server.uri(), None);
    let result = super::detect_and_build_client(&ctx, &tls_config, &mut config).await;
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
    let ctx = connect_context("test", &server.uri(), Some(crate::types::ApiMode::XmlRpc));
    let result = super::detect_and_build_client(&ctx, &tls_config, &mut config).await;
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
    let ctx = connect_context("test", "https://127.0.0.1:1", None);
    let result = super::handle_tofu(&ctx, &mut config).await;
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
    let ctx = connect_context("test", "https://example.test", None);
    let result = super::handle_pin_rotation(
        &ctx,
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
    let ctx = connect_context("test", &server.uri(), None);
    let result = super::detect_with_tofu_fallback(&ctx, &tls_config, &mut config).await;
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
}

#[test]
fn tls_uses_default_trust_true_for_default_config() {
    assert!(super::tls_uses_default_trust(&TlsConfig::default()));
}

#[test]
fn tls_uses_default_trust_false_when_insecure() {
    let tls = TlsConfig {
        insecure: true,
        ..Default::default()
    };
    assert!(!super::tls_uses_default_trust(&tls));
}

#[test]
fn tls_uses_default_trust_false_when_ca_cert_set() {
    let tls = TlsConfig {
        ca_cert_path: Some("/path/to/ca.pem".into()),
        ..Default::default()
    };
    assert!(!super::tls_uses_default_trust(&tls));
}

#[test]
fn tls_uses_default_trust_false_when_pin_set() {
    let tls = TlsConfig {
        pin_sha256: Some("sha256//AAAA".into()),
        ..Default::default()
    };
    assert!(!super::tls_uses_default_trust(&tls));
}

/// Cached-path connect should probe TLS via a HEAD request whenever
/// verification is enabled, so TLS cert errors (TOFU, pin mismatch,
/// issuer change) surface at connect-time rather than being deferred to
/// the first real API call.
#[tokio::test]
async fn cached_path_probes_tls_when_default_trust() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    write_config(
        &tmp,
        &mock.uri(),
        "auth_method = \"header\"\napi_mode = \"rest\"",
    );

    // The probe sends a HEAD to the server URL. Wiremock returns 404 for
    // unmounted paths; the probe treats any non-transport error response
    // as "TLS handshake completed, no error to surface." We mount an
    // explicit expectation here to assert the probe actually fires.
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&mock)
        .await;

    let result = super::connect_and_configure(None, None).await;
    assert!(
        result.is_ok(),
        "cached path with default trust should still succeed after probe"
    );
    // Mock expectation verified on drop.
}

/// Pinned cached path must also probe — without this, a rotated cert
/// would only surface lazily from the first real API call, bypassing
/// the rotation prompt.
#[tokio::test]
async fn cached_path_probes_tls_when_pinned() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    // Cached config with a pinned fingerprint. Wiremock is HTTP, so the
    // pinned verifier is never invoked; the probe HEAD reaches the
    // server normally and returns 404.
    write_config(
        &tmp,
        &mock.uri(),
        "auth_method = \"header\"\napi_mode = \"rest\"\n\
         tls_pin_sha256 = \"sha256//AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=\"",
    );

    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&mock)
        .await;

    let result = super::connect_and_configure(None, None).await;
    assert!(
        result.is_ok(),
        "cached path with pinned cert should still succeed after probe"
    );
}

/// The probe must not follow HTTP redirects. If it did, a 301/302 from
/// the configured server URL to a different host would pin (or
/// `PIN_MISMATCH` against) the redirect target's certificate — i.e., the
/// prompt would describe one endpoint while validating another.
#[tokio::test]
async fn cached_path_probe_does_not_follow_redirects() {
    let _lock = ENV_LOCK.lock().await;
    let primary = MockServer::start().await;
    let secondary = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    write_config(
        &tmp,
        &primary.uri(),
        "auth_method = \"header\"\napi_mode = \"rest\"",
    );

    // Primary: 301 -> secondary URI. Secondary: any HEAD must NOT be
    // received (probe should treat the 301 as a completed handshake and
    // stop there).
    Mock::given(method("HEAD"))
        .respond_with(
            ResponseTemplate::new(301).insert_header("Location", secondary.uri().as_str()),
        )
        .expect(1)
        .mount(&primary)
        .await;
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&secondary)
        .await;

    let result = super::connect_and_configure(None, None).await;
    assert!(
        result.is_ok(),
        "probe should treat 301 as connect-success and not chase redirects"
    );
    // Secondary's expect(0) verified on drop — no hit means no redirect followed.
}

/// `classify_and_handle_tls_failure` must silently pass through non-TLS
/// transport errors so the cached-path probe doesn't block on transient
/// network issues. The actual command will surface the same error with
/// full request context.
#[tokio::test]
async fn classify_and_handle_tls_failure_returns_none_for_non_tls_error() {
    // Build a real reqwest::Error from a connection failure — error
    // chain contains no TLS markers, so all three predicates miss.
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

    let mut config = crate::config::Config::default();
    let tls_config = TlsConfig::default();
    let ctx = connect_context("test", "http://127.0.0.1:1/unreachable", None);
    let result =
        super::classify_and_handle_tls_failure(&bzr_err, &ctx, &tls_config, &mut config).await;
    match result {
        Ok(None) => {}
        Ok(Some(_)) => panic!("expected Ok(None) for non-TLS error, got Some(client)"),
        Err(e) => panic!("expected Ok(None) for non-TLS error, got Err: {e}"),
    }
}

/// `probe_tls` must return `Err` (wrapped in `BzrError::Http`) when the
/// underlying request fails on transport. The cached-path branch then
/// delegates to `classify_and_handle_tls_failure`, which for non-TLS
/// errors returns `Ok(None)` and the cached values flow through.
#[tokio::test]
async fn probe_tls_returns_err_on_unreachable_address() {
    let tls_config = TlsConfig::default();
    let result = super::probe_tls("http://127.0.0.1:1/unreachable", &tls_config).await;
    match result {
        Err(BzrError::Http(_)) => {}
        Err(other) => panic!("expected Http error, got {other:?}"),
        Ok(()) => panic!("expected probe to fail against unreachable address"),
    }
}

/// Cached-path with default trust where the probe fails on a non-TLS
/// transport error: the connect call should still succeed (returning
/// the cached client) because non-TLS probe failures are silent.
#[tokio::test]
async fn cached_path_proceeds_when_probe_fails_on_non_tls_error() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::TempDir::new().unwrap();
    // Point the configured server at an unreachable port so probe_tls
    // returns Err with a non-TLS connection error. classify_and_handle
    // _tls_failure should return Ok(None) for that, and the cached
    // path should fall through to building the client with cached
    // settings.
    write_config(
        &tmp,
        "http://127.0.0.1:1",
        "auth_method = \"header\"\napi_mode = \"rest\"",
    );

    let result = super::connect_and_configure(None, None).await;
    assert!(
        result.is_ok(),
        "non-TLS probe failures must not block the cached path"
    );
}

/// Cached-path connect should NOT probe when verification is explicitly
/// disabled — there is no TLS error class to surface in that mode.
#[tokio::test]
async fn cached_path_skips_probe_when_insecure() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    write_config(
        &tmp,
        &mock.uri(),
        "auth_method = \"header\"\napi_mode = \"rest\"\ntls_insecure = true",
    );

    // Assert HEAD never reaches the server when verification is off.
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let result = super::connect_and_configure(None, None).await;
    assert!(
        result.is_ok(),
        "cached path with insecure flag should skip probe"
    );
}
