#![expect(clippy::unwrap_used)]

use super::*;

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
