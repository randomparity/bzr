#![expect(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::error::BzrError;
use crate::test_helpers::{setup_isolated_env, write_config_to};
use crate::ENV_LOCK;

use super::test_helpers::{load_config, mount_detection_mocks, write_config};

/// A JSON-format command context pointed at an explicit config path, so connect
/// resolves config without `XDG_CONFIG_HOME` and the test needs no `ENV_LOCK`.
fn ctx_at(
    config_path: &Path,
    api: Option<crate::types::ApiMode>,
) -> crate::commands::runtime::context::CommandContext {
    crate::commands::runtime::context::CommandContext::new(
        None,
        crate::types::OutputFormat::Json,
        api,
    )
    .with_config_path_override(Some(config_path.to_path_buf()))
}

fn write_credentialless_config(tmp: &tempfile::TempDir, server_url: &str, extra: &str) -> PathBuf {
    let config_content = format!(
        r#"
default_server = "test"

[servers.test]
url = "{server_url}"
{extra}
"#,
    );
    write_config_to(tmp, &config_content)
}

#[tokio::test]
async fn connect_client_returns_client() {
    let (mock, _tmp, config_path) = setup_isolated_env().await;

    // whoami endpoint used by auth detection (already cached in setup_config)
    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .mount(&mock)
        .await;

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn connect_client_with_email_config_succeeds() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config(
        &tmp,
        &mock.uri(),
        "auth_method = \"header\"\napi_mode = \"rest\"\nemail = \"user@example.com\"",
    );

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .mount(&mock)
        .await;

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
    assert!(
        result.is_ok(),
        "connect_client with email config should succeed"
    );
}

#[tokio::test]
async fn connect_client_api_override_applies() {
    let (mock, _tmp, config_path) = setup_isolated_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .mount(&mock)
        .await;

    // Override with XmlRpc mode — connect should still succeed
    let result =
        super::connect_and_configure(&ctx_at(&config_path, Some(crate::types::ApiMode::XmlRpc)))
            .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn connect_client_missing_server_fails() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Config with no servers.
    let config_path = write_config_to(&tmp, "");

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
    assert!(result.is_err());
}

/// Exercises the full orchestration: no cached auth -> probes server -> persists result.
#[tokio::test]
async fn uncached_auth_detects_and_persists() {
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

    // Set up a real config file (uncached: no auth_method/api_mode) so detection
    // runs and update_locked persists back to it.
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config(&tmp, &server.uri(), "");

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
    assert!(result.is_ok(), "connect_client should succeed");

    // Verify persistence: reload from disk
    let reloaded = load_config(&config_path);
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
async fn credentialless_named_server_persists_api_mode_without_auth_method() {
    let server = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_credentialless_config(&tmp, &server.uri(), "");

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
    assert!(
        result.is_ok(),
        "credentialless named server should connect anonymously: {:?}",
        result.err()
    );

    let reloaded = load_config(&config_path);
    let srv = &reloaded.servers["test"];
    assert_eq!(srv.auth_method, None);
    assert_eq!(srv.api_mode, Some(crate::types::ApiMode::Rest));
    assert_eq!(srv.server_version.as_deref(), Some("5.1.2"));

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0]
        .headers
        .get(crate::bugzilla_auth::AUTH_HEADER_NAME)
        .is_none());
    assert!(requests[0]
        .url
        .query_pairs()
        .all(|(name, _)| name != crate::bugzilla_auth::AUTH_QUERY_PARAM));
}

#[tokio::test]
async fn credentialless_cached_mode_builds_anonymous_client() {
    let server = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_credentialless_config(&tmp, &server.uri(), "api_mode = \"rest\"");

    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(404))
        .expect(1)
        .mount(&server)
        .await;

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
    assert!(
        result.is_ok(),
        "credentialless cached mode should build an anonymous client: {:?}",
        result.err()
    );

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0]
        .headers
        .get(crate::bugzilla_auth::AUTH_HEADER_NAME)
        .is_none());
    assert!(requests[0]
        .url
        .query_pairs()
        .all(|(name, _)| name != crate::bugzilla_auth::AUTH_QUERY_PARAM));
}

#[tokio::test]
async fn connect_client_resolves_env_backed_api_key() {
    // Retains ENV_LOCK: this test mutates the process-global BZR_TEST_API_KEY
    // that api_key_env resolution reads. Config itself is selected by explicit
    // path, so no XDG_CONFIG_HOME mutation is needed.
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
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
    let config_path = write_config_to(&tmp, &config_content);
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe {
        std::env::set_var("BZR_TEST_API_KEY", "test-key");
    }

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .mount(&mock)
        .await;

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
    assert!(result.is_ok(), "env-backed config should succeed");
}

/// #314 acceptance: a complete connect against an inline `--server-url` server
/// works with NO config file present, and writes none to disk.
#[tokio::test]
async fn inline_server_connects_without_config_and_persists_nothing() {
    // Retains ENV_LOCK: this test mutates the process-global BZR_INLINE_TEST_KEY
    // that the inline api_key_env resolution reads.
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe {
        std::env::set_var("BZR_INLINE_TEST_KEY", "inline-secret");
    }
    mount_detection_mocks(&mock).await;

    let config_path = tmp.path().join("bzr").join("config.toml");
    let inline = crate::commands::runtime::inline_server::InlineServer {
        url: mock.uri(),
        api_key_env: Some("BZR_INLINE_TEST_KEY".into()),
        email: None,
        tls: crate::commands::runtime::inline_server::InlineTlsOptions::default(),
    };
    let ctx = ctx_at(&config_path, None).with_inline_server(Some(inline));
    let result = super::connect_and_configure(&ctx).await;

    assert!(
        result.is_ok(),
        "inline server should connect with no config file: {:?}",
        result.err()
    );
    assert!(
        !config_path.exists(),
        "an inline connect must not create or write the config file"
    );
}

#[tokio::test]
async fn inline_credentialless_server_connects_without_config() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let config_path = tmp.path().join("bzr").join("config.toml");
    let inline = crate::commands::runtime::inline_server::InlineServer {
        url: mock.uri(),
        api_key_env: None,
        email: None,
        tls: crate::commands::runtime::inline_server::InlineTlsOptions::default(),
    };
    let ctx = ctx_at(&config_path, None).with_inline_server(Some(inline));
    let result = super::connect_and_configure(&ctx).await;

    assert!(
        result.is_ok(),
        "inline credentialless server should connect with no config file: {:?}",
        result.err()
    );
    assert!(
        !config_path.exists(),
        "an inline connect must not create or write the config file"
    );

    let requests = mock.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0]
        .headers
        .get(crate::bugzilla_auth::AUTH_HEADER_NAME)
        .is_none());
    assert!(requests[0]
        .url
        .query_pairs()
        .all(|(name, _)| name != crate::bugzilla_auth::AUTH_QUERY_PARAM));
}

/// An inline server whose API-key env var is unset fails with a clear config
/// error naming the variable — not a panic or a silent empty key.
#[tokio::test]
async fn inline_server_missing_env_var_is_clean_error() {
    // Retains ENV_LOCK: this test mutates the process-global BZR_INLINE_ABSENT_KEY
    // (removing it) that the inline api_key_env resolution reads.
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::TempDir::new().unwrap();
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe {
        std::env::remove_var("BZR_INLINE_ABSENT_KEY");
    }

    let config_path = tmp.path().join("bzr").join("config.toml");
    let inline = crate::commands::runtime::inline_server::InlineServer {
        url: "https://bugzilla.example.com".into(),
        api_key_env: Some("BZR_INLINE_ABSENT_KEY".into()),
        email: None,
        tls: crate::commands::runtime::inline_server::InlineTlsOptions::default(),
    };
    let ctx = ctx_at(&config_path, None).with_inline_server(Some(inline));
    let result = super::connect_and_configure(&ctx).await;

    match result {
        Err(BzrError::Config(msg)) => {
            assert!(
                msg.contains("BZR_INLINE_ABSENT_KEY"),
                "error should name the missing env var: {msg}"
            );
        }
        Err(other) => panic!("expected a Config error for the unset env var, got {other:?}"),
        Ok(_) => panic!("expected an error for the unset env var, got a client"),
    }
}

/// `tls_insecure = true` should emit a warning and still complete the
/// connect flow, exercising the warn branch around the insecure flag.
#[tokio::test]
async fn connect_client_with_tls_insecure_warns_and_succeeds() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config(
        &tmp,
        &mock.uri(),
        "auth_method = \"header\"\napi_mode = \"rest\"\ntls_insecure = true",
    );
    mount_detection_mocks(&mock).await;

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
    assert!(result.is_ok(), "tls_insecure should still build a client");
}

/// The partial-cache branch must preserve the cached `auth_method` even
/// when re-detection would have picked a different method. Re-detection
/// runs to fill in the missing `api_mode` only.
#[tokio::test]
async fn connect_client_partial_cache_preserves_cached_auth_method() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    // Cache says "header"; live detection will reject header (401) and
    // accept query_param (200). The cached "header" must survive.
    let config_path = write_config(&tmp, &mock.uri(), "auth_method = \"header\"");

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

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
    assert!(
        result.is_ok(),
        "partial-cache with disagreeing detection should succeed: {:?}",
        result.err()
    );

    let reloaded = load_config(&config_path);
    let srv = &reloaded.servers["test"];
    assert_eq!(
        srv.auth_method,
        Some(crate::types::AuthMethod::Header),
        "cached auth_method must not be overwritten by re-detection"
    );
    assert_eq!(srv.api_mode, Some(crate::types::ApiMode::Rest));
}

/// Kill `ctx.api_key.is_none() → true` mutant (mod.rs:72): when a
/// credentialed server has `api_mode` cached but no `auth_method`, it must
/// take the detect path (hits whoami + version), NOT the anonymous probe path
/// (hits HEAD only). With the mutant, the guard is always `true`, so a
/// credentialed server would silently take the anonymous probe branch and skip
/// detection entirely.
#[tokio::test]
async fn credentialed_cached_mode_only_runs_detection_not_probe() {
    let server = wiremock::MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    // Credentialed server: has api_key + api_mode, but NO auth_method.
    // Real code: api_key.is_none() == false → falls to catch-all → detect.
    // Mutant: api_key.is_none() replaced by true → takes anonymous probe branch.
    let config_path = write_config(
        &tmp,
        &server.uri(),
        // api_mode cached, no auth_method — partial cache with credentials.
        "api_mode = \"rest\"",
    );

    // Detection mocks: expect whoami (auth detection) and version to be hit.
    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    // Probe mock: must NOT be called when api_key is present.
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(404))
        .expect(0)
        .mount(&server)
        .await;

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
    assert!(
        result.is_ok(),
        "credentialed server with only api_mode cached should detect and succeed: {:?}",
        result.err()
    );
}

/// `auth_method` cached but `api_mode` missing -> takes the partial-cache
/// branch in `connect_and_configure` (re-detects `api_mode`, persists
/// without overwriting `auth_method`).
#[tokio::test]
async fn connect_client_partial_cache_redetects_api_mode() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    // auth_method present, api_mode missing -> partial cache branch
    let config_path = write_config(&tmp, &mock.uri(), "auth_method = \"header\"");
    mount_detection_mocks(&mock).await;

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
    assert!(
        result.is_ok(),
        "partial-cache path should re-detect api_mode and succeed"
    );

    // Verify api_mode + version got persisted but auth_method stayed Header
    let reloaded = load_config(&config_path);
    let srv = &reloaded.servers["test"];
    assert_eq!(srv.auth_method, Some(crate::types::AuthMethod::Header));
    assert_eq!(srv.api_mode, Some(crate::types::ApiMode::Rest));
    assert_eq!(srv.server_version.as_deref(), Some("5.1.2"));
}

/// Cached-path connect should probe TLS via a HEAD request whenever
/// verification is enabled, so TLS cert errors (TOFU, pin mismatch,
/// issuer change) surface at connect-time rather than being deferred to
/// the first real API call.
#[tokio::test]
async fn cached_path_probes_tls_when_default_trust() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config(
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

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
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
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    // Cached config with a pinned fingerprint. Wiremock is HTTP, so the
    // pinned verifier is never invoked; the probe HEAD reaches the
    // server normally and returns 404.
    let config_path = write_config(
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

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
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
    let primary = MockServer::start().await;
    let secondary = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config(
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

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
    assert!(
        result.is_ok(),
        "probe should treat 301 as connect-success and not chase redirects"
    );
    // Secondary's expect(0) verified on drop — no hit means no redirect followed.
}

/// Cached-path with default trust where the probe fails on a non-TLS
/// transport error: the connect call should still succeed (returning
/// the cached client) because non-TLS probe failures are silent.
#[tokio::test]
async fn cached_path_proceeds_when_probe_fails_on_non_tls_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Point the configured server at an unreachable port so probe_tls
    // returns Err with a non-TLS connection error. classify_and_handle
    // _tls_failure should return Ok(None) for that, and the cached
    // path should fall through to building the client with cached
    // settings.
    let config_path = write_config(
        &tmp,
        "http://127.0.0.1:1",
        "auth_method = \"header\"\napi_mode = \"rest\"",
    );

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
    assert!(
        result.is_ok(),
        "non-TLS probe failures must not block the cached path"
    );
}

/// Cached-path connect should NOT probe when verification is explicitly
/// disabled — there is no TLS error class to surface in that mode.
#[tokio::test]
async fn cached_path_skips_probe_when_insecure() {
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config(
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

    let result = super::connect_and_configure(&ctx_at(&config_path, None)).await;
    assert!(
        result.is_ok(),
        "cached path with insecure flag should skip probe"
    );
}
