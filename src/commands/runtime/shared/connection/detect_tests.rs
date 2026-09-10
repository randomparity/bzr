#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::tls::TlsConfig;

use super::super::test_helpers::{
    connect_context, load_config, mount_detection_mocks, write_config,
};

#[tokio::test]
async fn detect_with_tofu_fallback_normal_path() {
    let server = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config(&tmp, &server.uri(), "");

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

    let tls_config = TlsConfig::default();
    let ctx = connect_context("test", &server.uri(), None, Some(config_path));
    let result = super::detect_with_tofu_fallback(&ctx, &tls_config).await;
    assert!(result.is_ok(), "normal path should succeed");
}

#[tokio::test]
async fn persist_detected_settings_skips_unknown_server() {
    // If the server name doesn't exist in config, persist is a no-op.
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config(&tmp, "https://example.test", "");

    let settings = crate::client::DetectedServerSettings {
        auth_method: Some(crate::types::AuthMethod::Header),
        api_mode: crate::types::ApiMode::Rest,
        server_version: Some("5.1".into()),
        auth_method_probed: true,
    };
    let result =
        super::persist_detected_settings(Some(&config_path), "nonexistent", &settings, true);
    assert!(result.is_ok());

    // The known server is untouched and no "nonexistent" server is created.
    let reloaded = load_config(&config_path);
    assert!(!reloaded.servers.contains_key("nonexistent"));
}

/// When the version probe failed (`server_version` is `None`), persistence
/// must NOT overwrite a previously-cached `api_mode` — a transient version
/// blip must not clobber a known-good mode. Only `auth_method` (when
/// `persist_auth`) may be written. Locks the "persist only when intended"
/// invariant in `persist_detected_settings`.
#[tokio::test]
async fn persist_skips_api_mode_when_version_probe_failed() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Seed config with a known-good cached api_mode.
    let config_path = write_config(&tmp, "https://example.test", "api_mode = \"rest\"");

    let settings = crate::client::DetectedServerSettings {
        auth_method: Some(crate::types::AuthMethod::Header),
        // Hybrid is the fallback mode produced when the version probe fails; it
        // would clobber the cached Rest mode if persisted.
        api_mode: crate::types::ApiMode::Hybrid,
        server_version: None,
        auth_method_probed: false,
    };
    super::persist_detected_settings(Some(&config_path), "test", &settings, false).unwrap();

    let reloaded = load_config(&config_path);
    let srv = &reloaded.servers["test"];
    assert_eq!(
        srv.api_mode,
        Some(crate::types::ApiMode::Rest),
        "cached api_mode must survive a failed version probe"
    );
    assert_eq!(
        srv.auth_method, None,
        "persist_auth=false must not write an auth_method"
    );
}

// ── provenance stamp and change warning (ADR-0066) ────────────────

/// Persist `detected` over a `[servers.test]` entry carrying `extra`, and
/// return the reloaded entry plus everything the run logged at `warn`.
fn persist_and_capture(
    extra: &str,
    detected: Option<crate::types::AuthMethod>,
    persist_auth: bool,
) -> (crate::config::ServerConfig, String) {
    persist_and_capture_with_version(extra, detected, persist_auth, Some("5.2".into()), true)
}

/// As [`persist_and_capture`], with control over `server_version` — the local
/// signal for whether detection actually reached the server.
fn persist_and_capture_with_version(
    extra: &str,
    detected: Option<crate::types::AuthMethod>,
    persist_auth: bool,
    server_version: Option<String>,
    probed: bool,
) -> (crate::config::ServerConfig, String) {
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config(&tmp, "https://example.test", extra);
    let settings = crate::client::DetectedServerSettings {
        auth_method: detected,
        api_mode: crate::types::ApiMode::Rest,
        server_version,
        auth_method_probed: probed,
    };

    let (capture, guard) = crate::test_helpers::TracingCapture::install(tracing::Level::WARN);
    super::persist_detected_settings(Some(&config_path), "test", &settings, persist_auth).unwrap();
    drop(guard);

    let reloaded = load_config(&config_path);
    (reloaded.servers["test"].clone(), capture.output())
}

#[test]
fn persist_detected_stamps_the_detection_marker() {
    let (srv, _) = persist_and_capture("", Some(crate::types::AuthMethod::QueryParam), true);
    assert_eq!(srv.auth_method, Some(crate::types::AuthMethod::QueryParam));
    assert_eq!(
        srv.auth_method_source.as_deref(),
        Some(crate::config::AUTH_METHOD_SOURCE_DETECTED),
        "a persisted detection must stamp its provenance, or the next connect re-detects again"
    );
    assert!(
        srv.auth_method_is_trusted(),
        "the stamp it writes must be one it trusts on the next load"
    );
}

#[test]
fn persist_detected_warns_only_when_the_method_changed() {
    let (srv, changed) = persist_and_capture(
        "auth_method = \"header\"",
        Some(crate::types::AuthMethod::QueryParam),
        true,
    );
    assert_eq!(srv.auth_method, Some(crate::types::AuthMethod::QueryParam));
    assert!(
        changed.contains("query_param") && changed.contains("header"),
        "the warn must name both the new and the old method: {changed}"
    );
    assert!(
        changed.contains("--auth-method header"),
        "the warn must name the command that pins the old method back: {changed}"
    );
    assert_eq!(
        changed.matches("re-detected auth method").count(),
        1,
        "exactly one warn per change: {changed}"
    );

    // Confirming the cached value is the common case and must stay quiet.
    let (_, unchanged) = persist_and_capture(
        "auth_method = \"header\"",
        Some(crate::types::AuthMethod::Header),
        true,
    );
    assert!(
        !unchanged.contains("re-detected auth method"),
        "re-detection that confirms the value must not warn: {unchanged}"
    );

    // Nor must a server's first-ever detection, which overturns nothing.
    let (_, first) = persist_and_capture("", Some(crate::types::AuthMethod::QueryParam), true);
    assert!(
        !first.contains("re-detected auth method"),
        "a first detection must not warn: {first}"
    );
}

#[test]
fn persist_detected_leaves_the_stamp_alone_when_auth_is_not_persisted() {
    // The partial-cache arm re-detects only to fill in `api_mode`. It must not
    // restamp, or a `"pinned"` marker would silently become a detected one.
    let (srv, _) = persist_and_capture(
        "auth_method = \"header\"\nauth_method_source = \"pinned\"",
        Some(crate::types::AuthMethod::QueryParam),
        false,
    );
    assert_eq!(srv.auth_method, Some(crate::types::AuthMethod::Header));
    assert_eq!(
        srv.auth_method_source.as_deref(),
        Some(crate::config::AUTH_METHOD_SOURCE_PINNED)
    );
}

#[test]
fn persist_detected_does_not_stamp_an_unreachable_server() {
    // `detect_auth_method` answers `header` without probing when the server is
    // unreachable, so stamping it would mark a guess as probe-derived and
    // permanently trust it — re-creating the stale-header state ADR-0066 ends.
    // A failed version probe is the local signal that this happened.
    let (srv, _) = persist_and_capture_with_version(
        "auth_method = \"header\"",
        Some(crate::types::AuthMethod::Header),
        true,
        None,
        false,
    );
    assert_eq!(
        srv.auth_method_source, None,
        "a fallback method from an unreachable server must stay unstamped"
    );
    assert!(
        !srv.auth_method_is_trusted(),
        "leaving it unstamped is what makes the next connect retry detection"
    );
}

/// A genuinely probed method is stamped even when the version probe failed — the
/// gate is `auth_method_probed`, not `server_version` (ADR-0069).
#[test]
fn probed_method_without_version_is_stamped() {
    let (srv, _) = persist_and_capture_with_version(
        "",
        Some(crate::types::AuthMethod::QueryParam),
        true,
        None,
        true,
    );
    assert_eq!(
        srv.auth_method_source.as_deref(),
        Some(crate::config::AUTH_METHOD_SOURCE_DETECTED),
        "a genuinely probed method must be stamped even when the version probe failed"
    );
}

/// A transport-fallback method is NOT stamped even when a version is present —
/// the dangerous case ADR-0066 left open, now closed by the probe-outcome gate.
#[test]
fn fallback_method_with_version_is_not_stamped() {
    let (srv, _) = persist_and_capture_with_version(
        "",
        Some(crate::types::AuthMethod::Header),
        true,
        Some("5.1".into()),
        false,
    );
    assert_eq!(
        srv.auth_method_source, None,
        "a fallback method must stay unstamped even when a version is present, or it is permanently trusted"
    );
    assert!(
        !srv.auth_method_is_trusted(),
        "leaving it unstamped is what makes the next connect retry detection"
    );
}

/// A pre-stamped (trusted) entry that re-detects with a transport fallback is
/// CLEARED of its stamp — a fallback must never sit under a stale trusted marker
/// (the TOFU/pin-rotation clobber path, ADR-0069).
#[test]
fn unprobed_fallback_clears_a_stale_trusted_stamp() {
    for existing in [
        crate::config::AUTH_METHOD_SOURCE_DETECTED,
        crate::config::AUTH_METHOD_SOURCE_PINNED,
    ] {
        let (srv, _) = persist_and_capture_with_version(
            &format!("auth_method_source = \"{existing}\"\n"),
            Some(crate::types::AuthMethod::Header),
            true,
            Some("5.1".into()),
            false,
        );
        assert_eq!(
            srv.auth_method_source, None,
            "a transport fallback must clear a stale {existing} stamp, or it is permanently trusted"
        );
        assert!(!srv.auth_method_is_trusted());
    }
}

#[test]
fn persist_detected_leaves_the_stamp_alone_for_an_anonymous_detection() {
    // Credentialless detection yields no auth method; it must not stamp a
    // provenance for a value it did not produce.
    let (srv, _) = persist_and_capture("", None, true);
    assert_eq!(srv.auth_method, None);
    assert_eq!(srv.auth_method_source, None);
}

/// `detect_and_build_client` is the shared tail of the TOFU/rotation
/// flows: detect → persist → construct client. Drive it with a real
/// wiremock to cover lines 211-231.
#[tokio::test]
async fn detect_and_build_client_persists_and_returns_client() {
    let server = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config(&tmp, &server.uri(), "");
    mount_detection_mocks(&server).await;

    let tls_config = TlsConfig::default();
    let ctx = connect_context("test", &server.uri(), None, Some(config_path.clone()));
    let result = super::detect_and_build_client(&ctx, &tls_config).await;
    assert!(result.is_ok(), "detect_and_build_client should succeed");

    // Verify the settings were persisted.
    let reloaded = load_config(&config_path);
    let srv = &reloaded.servers["test"];
    assert_eq!(srv.auth_method, Some(crate::types::AuthMethod::Header));
    assert_eq!(srv.api_mode, Some(crate::types::ApiMode::Rest));
}

/// `detect_and_build_client` should honor an `api_override` even when the
/// server's detected mode would be different.
#[tokio::test]
async fn detect_and_build_client_respects_api_override() {
    let server = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config(&tmp, &server.uri(), "");
    mount_detection_mocks(&server).await;

    let tls_config = TlsConfig::default();
    let ctx = connect_context(
        "test",
        &server.uri(),
        Some(crate::types::ApiMode::XmlRpc),
        Some(config_path),
    );
    let result = super::detect_and_build_client(&ctx, &tls_config).await;
    assert!(result.is_ok(), "api_override should still produce a client");
}

/// `detect_with_tofu_fallback` should propagate non-TLS errors as-is
/// (covers the catch-all `Err(e) => Err(e)` branch around line 281).
#[tokio::test]
async fn detect_with_tofu_fallback_propagates_auth_errors() {
    let server = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = write_config(&tmp, &server.uri(), "");

    // No mocks mounted -> wiremock returns 404 for every request.
    // detect_auth_method will exhaust whoami + valid_login (no email)
    // and return BzrError::Auth, which is not a TLS error -> propagates.
    let tls_config = TlsConfig::default();
    let ctx = connect_context("test", &server.uri(), None, Some(config_path));
    let result = super::detect_with_tofu_fallback(&ctx, &tls_config).await;
    assert!(result.is_err(), "auth failure should propagate");
}
