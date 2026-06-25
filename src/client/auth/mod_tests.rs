#![expect(clippy::unwrap_used)]

use std::io::Read as _;
use std::net::TcpListener;
use std::sync::Arc;

use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::bugzilla_auth::AUTH_HEADER_NAME;
use crate::client::test_helpers::test_http_client;
use crate::error::BzrError;

fn spawn_self_signed_https_server() -> (String, std::thread::JoinHandle<()>) {
    let params = rcgen::CertificateParams::new(vec!["localhost".to_owned()]).unwrap();
    let key_pair = rcgen::KeyPair::generate().unwrap();
    let cert = params.self_signed(&key_pair).unwrap();
    let cert_der = rustls::pki_types::CertificateDer::from(cert.der().to_vec());
    let key_der = rustls::pki_types::PrivateKeyDer::Pkcs8(
        rustls::pki_types::PrivatePkcs8KeyDer::from(key_pair.serialize_der()),
    );
    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert_der], key_der)
        .unwrap();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let Ok((tcp, _addr)) = listener.accept() else {
            return;
        };
        let Ok(connection) = rustls::ServerConnection::new(Arc::new(server_config)) else {
            return;
        };
        let mut stream = rustls::StreamOwned::new(connection, tcp);
        let _ = stream.read(&mut [0_u8; 1]);
    });

    (format!("https://localhost:{port}"), handle)
}

#[test]
fn trace_body_preview_caps_long_auth_probe_bodies() {
    let body = "x".repeat(AUTH_PROBE_BODY_TRACE_MAX_BYTES + 1);

    let preview = trace_body_preview(&body);

    assert_eq!(preview.len(), AUTH_PROBE_BODY_TRACE_MAX_BYTES);
    assert_eq!(preview, "x".repeat(AUTH_PROBE_BODY_TRACE_MAX_BYTES));
}

#[test]
fn trace_body_preview_stops_on_utf8_boundary() {
    let mut body = "a".repeat(AUTH_PROBE_BODY_TRACE_MAX_BYTES - 1);
    body.push('é');
    body.push_str("trailing");

    let preview = trace_body_preview(&body);

    assert_eq!(preview, "a".repeat(AUTH_PROBE_BODY_TRACE_MAX_BYTES - 1));
    assert!(preview.len() <= AUTH_PROBE_BODY_TRACE_MAX_BYTES);
}

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
        .and(query_param(
            crate::bugzilla_auth::AUTH_QUERY_PARAM,
            "test-key",
        ))
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
        .and(query_param(
            crate::bugzilla_auth::AUTH_QUERY_PARAM,
            "test-key",
        ))
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
        .and(query_param(
            crate::bugzilla_auth::AUTH_QUERY_PARAM,
            "test-key",
        ))
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
async fn whoami_malformed_200_adds_parse_context_to_auth_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .and(query_param(
            crate::bugzilla_auth::AUTH_QUERY_PARAM,
            "test-key",
        ))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let result = detect_auth_method(&test_http_client(), &server.uri(), "test-key", None).await;
    assert!(matches!(result, Err(BzrError::Auth(_))));
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("whoami"),
        "should name the malformed probe, got: {err}"
    );
    assert!(
        err.contains("header"),
        "should name the malformed auth method, got: {err}"
    );
    assert!(
        err.contains("malformed"),
        "should describe the malformed response, got: {err}"
    );
    assert!(
        err.contains("expected"),
        "should include the parser error, got: {err}"
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
async fn anonymous_detection_propagates_tls_certificate_errors() {
    let (url, handle) = spawn_self_signed_https_server();

    let result = detect_server_settings_without_auth(
        &url,
        &crate::tls::TlsConfig::default(),
        crate::http::REQUEST_TIMEOUT,
    )
    .await;
    handle.join().unwrap();

    assert!(
        matches!(&result, Err(BzrError::Http(_))),
        "expected TLS HTTP error from anonymous detection, got: {result:?}"
    );
    let Err(BzrError::Http(err)) = result else {
        return;
    };
    assert!(
        crate::tls::is_tls_cert_error(&err),
        "anonymous detection should surface TLS cert errors, got: {err:#}"
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
async fn valid_login_malformed_200_adds_parse_context_to_auth_error() {
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
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(query_param("login", "user@example.com"))
        .and(query_param(
            crate::bugzilla_auth::AUTH_QUERY_PARAM,
            "test-key",
        ))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let result = detect_auth_method(
        &test_http_client(),
        &server.uri(),
        "test-key",
        Some("user@example.com"),
    )
    .await;
    assert!(matches!(result, Err(BzrError::Auth(_))));
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("valid_login"),
        "should name the malformed probe, got: {err}"
    );
    assert!(
        err.contains("header"),
        "should name the malformed auth method, got: {err}"
    );
    assert!(
        err.contains("malformed"),
        "should describe the malformed response, got: {err}"
    );
    assert!(
        err.contains("expected"),
        "should include the parser error, got: {err}"
    );
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
        crate::http::REQUEST_TIMEOUT,
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
        crate::http::REQUEST_TIMEOUT,
    )
    .await
    .unwrap();
    assert_eq!(detected.auth_method, Some(AuthMethod::Header));
    assert_eq!(detected.api_mode, ApiMode::Hybrid);
    assert!(detected.server_version.is_none());
}
