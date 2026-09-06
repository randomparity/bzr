#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::client::test_helpers::test_http_client;

#[test]
fn version_to_mode_pre_5() {
    assert_eq!(version_to_api_mode("4.4.13"), ApiMode::XmlRpc);
    assert_eq!(version_to_api_mode("3.6.1"), ApiMode::XmlRpc);
}

#[test]
fn version_to_mode_5_0() {
    assert_eq!(version_to_api_mode("5.0"), ApiMode::Hybrid);
    assert_eq!(version_to_api_mode("5.0+"), ApiMode::Hybrid);
    assert_eq!(version_to_api_mode("5.0.4"), ApiMode::Hybrid);
    assert_eq!(version_to_api_mode("5.0.4.rh103"), ApiMode::Hybrid);
}

#[test]
fn version_to_mode_5_1_plus() {
    assert_eq!(version_to_api_mode("5.1"), ApiMode::Rest);
    assert_eq!(version_to_api_mode("5.1+"), ApiMode::Rest);
    assert_eq!(version_to_api_mode("5.2+"), ApiMode::Rest);
    assert_eq!(version_to_api_mode("5.1.2"), ApiMode::Rest);
    assert_eq!(version_to_api_mode("5.3.3+"), ApiMode::Rest);
    assert_eq!(version_to_api_mode("6.0"), ApiMode::Rest);
}

#[test]
fn version_to_mode_rejects_non_bare_minor_suffixes() {
    assert_eq!(version_to_api_mode("5.1++"), ApiMode::Hybrid);
    assert_eq!(version_to_api_mode("5.1+.2"), ApiMode::Hybrid);
}

#[tokio::test]
async fn detect_version_returns_rest_for_5_1() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .mount(&server)
        .await;

    let (version, mode) = detect_version_and_mode(
        &test_http_client(),
        &server.uri(),
        "test-key",
        AuthMethod::Header,
    )
    .await;
    assert_eq!(version.as_deref(), Some("5.1.2"));
    assert_eq!(mode, ApiMode::Rest);
}

#[tokio::test]
async fn detect_version_and_mode_without_auth_sends_no_credentials() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .expect(1)
        .mount(&server)
        .await;

    let result =
        detect_version_and_mode_without_auth_checked(&test_http_client(), &server.uri()).await;
    assert!(
        result.is_ok(),
        "anonymous version probe should succeed: {result:?}"
    );
    let Ok((version, mode)) = result else {
        return;
    };

    assert_eq!(version.as_deref(), Some("5.1.2"));
    assert_eq!(mode, ApiMode::Rest);

    let requests = server.received_requests().await.unwrap_or_default();
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
async fn detect_version_returns_hybrid_for_5_0() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.0.4"})),
        )
        .mount(&server)
        .await;

    let (version, mode) = detect_version_and_mode(
        &test_http_client(),
        &server.uri(),
        "test-key",
        AuthMethod::Header,
    )
    .await;
    assert_eq!(version.as_deref(), Some("5.0.4"));
    assert_eq!(mode, ApiMode::Hybrid);
}

#[tokio::test]
async fn detect_version_404_returns_xmlrpc() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let (version, mode) = detect_version_and_mode(
        &test_http_client(),
        &server.uri(),
        "test-key",
        AuthMethod::Header,
    )
    .await;
    assert!(version.is_none());
    assert_eq!(mode, ApiMode::XmlRpc);
}

#[tokio::test]
async fn detect_version_network_error_returns_xmlrpc() {
    let (version, mode) = detect_version_and_mode(
        &test_http_client(),
        "https://127.0.0.1:1",
        "test-key",
        AuthMethod::Header,
    )
    .await;
    assert!(version.is_none());
    assert_eq!(mode, ApiMode::XmlRpc);
}

/// `detect_version_and_mode_without_auth_checked` uses `PropagateTlsCertificate`
/// mode and should only propagate errors that are TLS certificate errors.
/// A plain connection-refused failure is NOT a TLS error, so it must be
/// swallowed and returned as `Ok((None, XmlRpc))`.
///
/// The `&&` → `||` mutant would cause this function to propagate ANY send
/// error in `PropagateTlsCertificate` mode (treating the first operand alone
/// as sufficient), making it return `Err` here instead of `Ok`.
#[tokio::test]
async fn detect_version_without_auth_connection_refused_returns_xmlrpc() {
    let result =
        detect_version_and_mode_without_auth_checked(&test_http_client(), "http://127.0.0.1:1")
            .await;
    assert!(
        result.is_ok(),
        "non-TLS connection failure must not propagate in PropagateTlsCertificate mode: {result:?}"
    );
    let Ok((version, mode)) = result else { return };
    assert!(version.is_none());
    assert_eq!(mode, ApiMode::XmlRpc);
}

/// A bare major version "5" with no minor part must still map to Hybrid,
/// matching the `(Some(5), None)` arm.  Without that arm it would fall to
/// `(Some(_), _)` and return Rest instead.
#[test]
fn version_to_mode_5_bare_no_minor_maps_to_hybrid() {
    assert_eq!(version_to_api_mode("5"), ApiMode::Hybrid);
}

/// Confirm the bare-"5" mapping is distinct from the 5.1+ Rest mapping.
#[test]
fn version_to_mode_5_bare_differs_from_5_1() {
    assert_ne!(version_to_api_mode("5"), version_to_api_mode("5.1"));
}

#[test]
fn parse_major_minor_reads_three_part_version() {
    assert_eq!(parse_major_minor("5.3.3+"), (Some(5), Some(3)));
    assert_eq!(parse_major_minor("5.0.6"), (Some(5), Some(0)));
    assert_eq!(parse_major_minor("5.2"), (Some(5), Some(2)));
}

#[test]
fn parse_major_minor_reads_two_part_plus_suffixed_version() {
    assert_eq!(parse_major_minor("5.3+"), (Some(5), Some(3)));
}

#[test]
fn parse_major_minor_minor_absent_when_bare_major() {
    assert_eq!(parse_major_minor("5"), (Some(5), None));
}

#[test]
fn parse_major_minor_both_absent_when_unparseable() {
    assert_eq!(parse_major_minor("unknown"), (None, None));
}

#[tokio::test]
async fn detect_version_non_json_returns_hybrid() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let (version, mode) = detect_version_and_mode(
        &test_http_client(),
        &server.uri(),
        "test-key",
        AuthMethod::Header,
    )
    .await;
    assert!(version.is_none());
    assert_eq!(mode, ApiMode::Hybrid);
}

#[tokio::test]
async fn version_probe_failure_message_does_not_leak_the_api_key() {
    // This probe runs immediately after auth detection with whatever method it
    // chose, and under QueryParam the key is in the URL. The caller logs at
    // `warn`, the default filter level, so an unredacted render needs no
    // verbosity flag to reach stderr.
    let error = test_http_client()
        .get("http://127.0.0.1:1/rest/version")
        .query(&[(crate::bugzilla_auth::AUTH_QUERY_PARAM, "super-secret-key")])
        .send()
        .await
        .unwrap_err();

    let message = version_probe_failure_message(&error);

    assert!(
        !message.contains("super-secret-key"),
        "version probe failure leaked the API key: {message}"
    );
    assert!(
        !message.contains(crate::bugzilla_auth::AUTH_QUERY_PARAM),
        "version probe failure leaked the auth query parameter: {message}"
    );
    assert!(
        message.contains("falling back to xmlrpc"),
        "message should still say what happened: {message}"
    );
    assert!(
        message.contains("http://127.0.0.1:1/rest/version"),
        "message should keep origin and path for diagnosis: {message}"
    );
}
