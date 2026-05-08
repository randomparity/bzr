#![expect(clippy::unwrap_used)]

use super::*;

pub fn test_http_client() -> reqwest::Client {
    crate::tls::build_tls_client(&crate::tls::TlsConfig::default()).unwrap()
}

pub fn test_client(base_url: &str) -> BugzillaClient {
    BugzillaClient::new(BugzillaClientConfig {
        base_url,
        credential: "test-key",
        auth_method: AuthMethod::Header,
        api_mode: ApiMode::Rest,
        email_hint: None,
        tls_config: &crate::tls::TlsConfig::default(),
    })
    .unwrap()
}

pub fn test_client_hybrid(base_url: &str) -> BugzillaClient {
    BugzillaClient::new(BugzillaClientConfig {
        base_url,
        credential: "test-key",
        auth_method: AuthMethod::Header,
        api_mode: ApiMode::Hybrid,
        email_hint: None,
        tls_config: &crate::tls::TlsConfig::default(),
    })
    .unwrap()
}

pub fn test_client_query_param(base_url: &str) -> BugzillaClient {
    BugzillaClient::new(BugzillaClientConfig {
        base_url,
        credential: "test-key",
        auth_method: AuthMethod::QueryParam,
        api_mode: ApiMode::Rest,
        email_hint: None,
        tls_config: &crate::tls::TlsConfig::default(),
    })
    .unwrap()
}

pub fn test_client_xmlrpc(base_url: &str) -> BugzillaClient {
    BugzillaClient::new(BugzillaClientConfig {
        base_url,
        credential: "test-key",
        auth_method: AuthMethod::Header,
        api_mode: ApiMode::XmlRpc,
        email_hint: None,
        tls_config: &crate::tls::TlsConfig::default(),
    })
    .unwrap()
}
