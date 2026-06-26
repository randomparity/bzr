#![expect(clippy::unwrap_used)]

use super::*;

pub fn test_http_client() -> reqwest::Client {
    crate::tls::build_tls_client(
        &crate::tls::TlsConfig::default(),
        crate::http::REQUEST_TIMEOUT,
    )
    .unwrap()
}

pub fn test_client(base_url: &str) -> BugzillaClient {
    BugzillaClient::new(BugzillaClientConfig {
        base_url,
        credential: Some("test-key"),
        auth_method: Some(AuthMethod::Header),
        api_mode: ApiMode::Rest,
        email_hint: None,
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
    })
    .unwrap()
}

pub fn test_client_hybrid(base_url: &str) -> BugzillaClient {
    BugzillaClient::new(BugzillaClientConfig {
        base_url,
        credential: Some("test-key"),
        auth_method: Some(AuthMethod::Header),
        api_mode: ApiMode::Hybrid,
        email_hint: None,
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
    })
    .unwrap()
}

pub fn test_client_anon(base_url: &str) -> BugzillaClient {
    BugzillaClient::new(BugzillaClientConfig {
        base_url,
        credential: None,
        auth_method: None,
        api_mode: ApiMode::Rest,
        email_hint: None,
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
    })
    .unwrap()
}

pub fn test_client_query_param(base_url: &str) -> BugzillaClient {
    BugzillaClient::new(BugzillaClientConfig {
        base_url,
        credential: Some("test-key"),
        auth_method: Some(AuthMethod::QueryParam),
        api_mode: ApiMode::Rest,
        email_hint: None,
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
    })
    .unwrap()
}

pub fn test_client_xmlrpc(base_url: &str) -> BugzillaClient {
    BugzillaClient::new(BugzillaClientConfig {
        base_url,
        credential: Some("test-key"),
        auth_method: Some(AuthMethod::Header),
        api_mode: ApiMode::XmlRpc,
        email_hint: None,
        tls_config: &crate::tls::TlsConfig::default(),
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
    })
    .unwrap()
}
