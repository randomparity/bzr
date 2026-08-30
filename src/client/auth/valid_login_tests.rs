#![expect(clippy::unwrap_used)]

use super::*;
use crate::client::PreparedAuth;
use crate::error::BzrError;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn strict_http_client() -> reqwest::Client {
    crate::tls::build_no_redirect_tls_client(
        &crate::tls::TlsConfig::default(),
        crate::http::REQUEST_TIMEOUT,
    )
    .unwrap()
}

#[test]
fn valid_login_result_from_bool_true() {
    let v: ValidLoginResult = serde_json::Value::Bool(true).try_into().unwrap();
    assert!(v.is_valid());
}

#[test]
fn valid_login_result_from_bool_false() {
    let v: ValidLoginResult = serde_json::Value::Bool(false).try_into().unwrap();
    assert!(!v.is_valid());
}

#[test]
fn valid_login_result_from_integer_1() {
    let v: ValidLoginResult = serde_json::json!(1).try_into().unwrap();
    assert!(v.is_valid());
}

#[test]
fn valid_login_result_from_integer_0() {
    let v: ValidLoginResult = serde_json::json!(0).try_into().unwrap();
    assert!(!v.is_valid());
}

#[test]
fn valid_login_result_from_string_errors() {
    let result: Result<ValidLoginResult, _> = serde_json::json!("yes").try_into();
    assert!(result.is_err());
}

#[test]
fn valid_login_response_deserializes() {
    let json = r#"{"result": true}"#;
    let resp: ValidLoginResponse = serde_json::from_str(json).unwrap();
    assert!(resp.result.is_valid());
}

#[test]
fn valid_login_response_integer_result() {
    let json = r#"{"result": 1}"#;
    let resp: ValidLoginResponse = serde_json::from_str(json).unwrap();
    assert!(resp.result.is_valid());
}

#[test]
fn valid_login_response_missing_result_errors() {
    let json = r"{}";
    let result = serde_json::from_str::<ValidLoginResponse>(json);
    assert!(result.is_err(), "missing result should fail to deserialize");
    let err = result.err().unwrap();
    assert!(
        err.to_string().contains("missing field `result`"),
        "unexpected error: {err}",
    );
}

#[tokio::test]
async fn current_header_proof_uses_only_the_configured_method() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(query_param("login", "user@example.com"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(query_param(AUTH_QUERY_PARAM, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})))
        .expect(0)
        .mount(&server)
        .await;

    prove_valid_login_current_method(
        &strict_http_client(),
        &server.uri(),
        "user@example.com",
        &PreparedAuth::Header(HeaderValue::from_static("test-key")),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn current_query_proof_uses_only_the_configured_method() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(query_param("login", "user@example.com"))
        .and(query_param(AUTH_QUERY_PARAM, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": 1})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/valid_login"))
        .and(header(AUTH_HEADER_NAME, "test-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})))
        .expect(0)
        .mount(&server)
        .await;

    prove_valid_login_current_method(
        &strict_http_client(),
        &server.uri(),
        "user@example.com",
        &PreparedAuth::QueryParam("test-key".to_owned()),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn current_proof_rejects_false_malformed_and_redirected_responses() {
    for (response, expected) in [
        (
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": false})),
            "did not confirm",
        ),
        (
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"error": true})),
            "invalid response",
        ),
        (
            ResponseTemplate::new(302).insert_header("location", "/landed"),
            "unexpected HTTP status",
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/valid_login"))
            .respond_with(response)
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/landed"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})),
            )
            .expect(0)
            .mount(&server)
            .await;

        let result = prove_valid_login_current_method(
            &strict_http_client(),
            &server.uri(),
            "user@example.com",
            &PreparedAuth::Header(HeaderValue::from_static("test-key")),
        )
        .await;

        assert!(matches!(result, Err(BzrError::Auth(_))));
        assert!(result.unwrap_err().to_string().contains(expected));
    }
}
