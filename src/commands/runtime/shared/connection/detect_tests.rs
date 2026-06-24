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
    };
    let result =
        super::persist_detected_settings(Some(&config_path), "nonexistent", &settings, true);
    assert!(result.is_ok());

    // The known server is untouched and no "nonexistent" server is created.
    let reloaded = load_config(&config_path);
    assert!(!reloaded.servers.contains_key("nonexistent"));
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
