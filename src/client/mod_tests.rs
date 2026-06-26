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
