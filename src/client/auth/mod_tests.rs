#![expect(clippy::unwrap_used)]

use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::client::test_helpers::test_http_client;
use crate::http::AUTH_HEADER_NAME;

#[tokio::test]
async fn header_auth_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 42})))
        .mount(&server)
        .await;

    let result = detect_auth_method(&test_http_client(), &server.uri(), "test-key", None)
        .await
        .unwrap();
    assert_eq!(result, AuthMethod::Header);
}

#[tokio::test]
async fn falls_back_to_query_param() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .and(query_param(crate::http::AUTH_QUERY_PARAM, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 7})))
        .mount(&server)
        .await;

    let result = detect_auth_method(&test_http_client(), &server.uri(), "test-key", None)
        .await
        .unwrap();
    assert_eq!(result, AuthMethod::QueryParam);
}

#[tokio::test]
async fn whoami_404_falls_back_to_valid_login_header() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(query_param("login", "user@example.com"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})))
        .mount(&server)
        .await;

    let result = detect_auth_method(
        &test_http_client(),
        &server.uri(),
        "test-key",
        Some("user@example.com"),
    )
    .await
    .unwrap();
    assert_eq!(result, AuthMethod::Header);
}

#[tokio::test]
async fn valid_login_query_param_but_header_works_on_api() {
    // Simulates IBM LTC-style servers: valid_login rejects header auth
    // but actual API endpoints accept it. Should prefer header.
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": false})),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(query_param("login", "user@example.com"))
        .and(query_param(crate::http::AUTH_QUERY_PARAM, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})))
        .mount(&server)
        .await;

    // Header auth works on real API endpoints
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&server)
        .await;

    let result = detect_auth_method(
        &test_http_client(),
        &server.uri(),
        "test-key",
        Some("user@example.com"),
    )
    .await
    .unwrap();
    assert_eq!(result, AuthMethod::Header);
}

#[tokio::test]
async fn valid_login_query_param_and_header_fails_on_api() {
    // Server truly only supports query_param: valid_login and API
    // endpoints both reject header auth.
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": false})),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(query_param("login", "user@example.com"))
        .and(query_param(crate::http::AUTH_QUERY_PARAM, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})))
        .mount(&server)
        .await;

    // Header auth rejected on real API endpoints
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let result = detect_auth_method(
        &test_http_client(),
        &server.uri(),
        "test-key",
        Some("user@example.com"),
    )
    .await
    .unwrap();
    assert_eq!(result, AuthMethod::QueryParam);
}

#[tokio::test]
async fn whoami_401_no_email_gives_api_key_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let result = detect_auth_method(&test_http_client(), &server.uri(), "test-key", None).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Check your API key"),
        "should mention API key, got: {err}"
    );
}

#[tokio::test]
async fn non_tls_network_error_defaults_to_header() {
    // A plain transport failure (connection refused — not a TLS cert error)
    // falls back to header auth rather than aborting: detection isn't retried,
    // and the real request (which is) may still succeed, so a transient
    // detection-time blip must not fail the whole invocation. TLS-certificate
    // failures take the opposite path (propagated so TOFU can fire) — exercised
    // via the connection layer's TLS classification tests.
    let result =
        detect_auth_method(&test_http_client(), "https://127.0.0.1:1", "test-key", None).await;
    assert!(
        matches!(result, Ok(AuthMethod::Header)),
        "non-TLS transport failure should default to header, got: {result:?}"
    );
}

#[tokio::test]
async fn whoami_404_no_email_suggests_email_flag() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let result = detect_auth_method(&test_http_client(), &server.uri(), "test-key", None).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("--email"),
        "should suggest --email flag, got: {err}"
    );
}

#[tokio::test]
async fn valid_login_accepts_integer_result() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(query_param("login", "user@example.com"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": 1})))
        .mount(&server)
        .await;

    let result = detect_auth_method(
        &test_http_client(),
        &server.uri(),
        "test-key",
        Some("user@example.com"),
    )
    .await
    .unwrap();
    assert_eq!(result, AuthMethod::Header);
}

#[tokio::test]
async fn both_methods_fail_with_email() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": false})),
        )
        .mount(&server)
        .await;

    let result = detect_auth_method(
        &test_http_client(),
        &server.uri(),
        "test-key",
        Some("bad@example.com"),
    )
    .await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    // When email IS provided but valid_login still fails, the hint
    // must point at the credentials, NOT suggest --email.
    assert!(
        err.contains("valid_login did not confirm"),
        "expected 'valid_login did not confirm' message; got: {err}"
    );
    assert!(
        !err.contains("--email"),
        "should not suggest --email when email was already provided; got: {err}"
    );
}

#[tokio::test]
async fn whoami_id_zero_is_auth_rejected() {
    // Bugzilla returns `id: 0` for an anonymous (unauthenticated)
    // whoami response. It must not be treated as a successful
    // authentication.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 0})))
        .mount(&server)
        .await;
    let result = detect_auth_method(&test_http_client(), &server.uri(), "test-key", None).await;
    assert!(result.is_err(), "id=0 must not authenticate");
}

/// Exercises `detect_server_settings`: probes auth and version without Config.
#[tokio::test]
async fn detect_server_settings_returns_all_fields() {
    let server = MockServer::start().await;

    // whoami succeeds with header auth
    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .mount(&server)
        .await;

    // version endpoint returns 5.1.2 -> REST mode
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .mount(&server)
        .await;

    let detected = detect_server_settings(
        &server.uri(),
        "test-key",
        None,
        &crate::tls::TlsConfig::default(),
    )
    .await
    .unwrap();
    assert_eq!(detected.auth_method, Some(AuthMethod::Header));
    assert_eq!(detected.api_mode, ApiMode::Rest);
    assert_eq!(detected.server_version.as_deref(), Some("5.1.2"));
}

#[tokio::test]
async fn invalid_api_key_characters_are_rejected() {
    let server = MockServer::start().await;
    let result = detect_auth_method(
        &test_http_client(),
        &server.uri(),
        "bad\nkey",
        Some("user@test"),
    )
    .await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("invalid API key characters"));
}

#[tokio::test]
async fn detect_server_settings_keeps_version_none_when_probe_fails() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .expect(1)
        .mount(&server)
        .await;

    let detected = detect_server_settings(
        &server.uri(),
        "test-key",
        None,
        &crate::tls::TlsConfig::default(),
    )
    .await
    .unwrap();
    assert_eq!(detected.auth_method, Some(AuthMethod::Header));
    assert_eq!(detected.api_mode, ApiMode::Hybrid);
    assert!(detected.server_version.is_none());
}
