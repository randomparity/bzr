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
    assert_eq!(version_to_api_mode("5.0.4"), ApiMode::Hybrid);
    assert_eq!(version_to_api_mode("5.0.4.rh103"), ApiMode::Hybrid);
}

#[test]
fn version_to_mode_5_1_plus() {
    assert_eq!(version_to_api_mode("5.1"), ApiMode::Rest);
    assert_eq!(version_to_api_mode("5.1.2"), ApiMode::Rest);
    assert_eq!(version_to_api_mode("6.0"), ApiMode::Rest);
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
