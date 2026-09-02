#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn id_response_deserializes_string_id() {
    let response: IdResponse = serde_json::from_str(r#"{"id":"42"}"#).unwrap();
    assert_eq!(response.id, 42);
}

#[test]
fn id_response_rejects_malformed_ids() {
    for json in [r#"{"id":"-1"}"#, r#"{"id":1.5}"#, r#"{"id":true}"#] {
        assert!(serde_json::from_str::<IdResponse>(json).is_err(), "{json}");
    }
}

#[test]
fn new_trims_trailing_slash_and_keeps_email_hint() {
    let client = BugzillaClient::new(BugzillaClientConfig {
        base_url: "https://bugzilla.example.com/",
        credential: Some("test-key"),
        auth_method: Some(AuthMethod::Header),
        api_mode: ApiMode::Rest,
        email_hint: Some("user@example.com"),
        server_name: "prod",
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
    })
    .unwrap();

    assert_eq!(client.base_url, "https://bugzilla.example.com");
    assert_eq!(client.email_hint.as_deref(), Some("user@example.com"));
    assert_eq!(client.server_name(), "prod");
}

#[test]
fn auth_mode_reflects_credential_presence() {
    let credentialed = test_helpers::test_client("https://bugzilla.example.com");
    assert_eq!(credentialed.auth_mode(), crate::types::AuthMode::ApiKey);

    let anonymous = test_helpers::test_client_anon("https://bugzilla.example.com");
    assert_eq!(anonymous.auth_mode(), crate::types::AuthMode::Anonymous);
}

#[tokio::test]
async fn new_retains_a_no_redirect_client_for_strict_operations() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/start"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/landed"))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/landed"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let client = test_helpers::test_client(&server.uri());
    let response = client
        .strict_http
        .get(format!("{}/start", server.uri()))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::FOUND);
}

#[tokio::test]
async fn current_credentials_valid_login_proof_requires_configured_email_and_credential() {
    let server = wiremock::MockServer::start().await;

    let no_email = test_helpers::test_client(&server.uri());
    assert!(matches!(
        no_email.prove_current_credentials().await,
        Err(BzrError::Auth(_))
    ));

    let anonymous = BugzillaClient::new(BugzillaClientConfig {
        base_url: &server.uri(),
        credential: None,
        auth_method: None,
        api_mode: ApiMode::Rest,
        email_hint: Some("user@example.com"),
        server_name: "test",
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
    })
    .unwrap();
    assert!(matches!(
        anonymous.prove_current_credentials().await,
        Err(BzrError::Auth(_))
    ));
    assert!(server.received_requests().await.unwrap().is_empty());

    let empty_email = BugzillaClient::new(BugzillaClientConfig {
        base_url: &server.uri(),
        credential: Some("test-key"),
        auth_method: Some(AuthMethod::Header),
        api_mode: ApiMode::Rest,
        email_hint: Some(""),
        server_name: "test",
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
    })
    .unwrap();
    assert!(matches!(
        empty_email.prove_current_credentials().await,
        Err(BzrError::Auth(_))
    ));
    assert!(server.received_requests().await.unwrap().is_empty());

    let empty_credential = BugzillaClient::new(BugzillaClientConfig {
        base_url: &server.uri(),
        credential: Some(""),
        auth_method: Some(AuthMethod::Header),
        api_mode: ApiMode::Rest,
        email_hint: Some("user@example.com"),
        server_name: "test",
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
    })
    .unwrap();
    assert!(matches!(
        empty_credential.prove_current_credentials().await,
        Err(BzrError::Auth(_))
    ));
    assert!(server.received_requests().await.unwrap().is_empty());
}
