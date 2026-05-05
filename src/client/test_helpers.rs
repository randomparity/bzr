#![expect(clippy::unwrap_used)]

use super::*;

pub fn test_http_client() -> reqwest::Client {
    crate::tls::build_tls_client(&crate::tls::TlsConfig::default()).unwrap()
}

pub fn test_client(base_url: &str) -> BugzillaClient {
    BugzillaClient::new(
        base_url,
        "test-key",
        AuthMethod::Header,
        ApiMode::Rest,
        None,
        &crate::tls::TlsConfig::default(),
    )
    .unwrap()
}

pub fn test_client_hybrid(base_url: &str) -> BugzillaClient {
    BugzillaClient::new(
        base_url,
        "test-key",
        AuthMethod::Header,
        ApiMode::Hybrid,
        None,
        &crate::tls::TlsConfig::default(),
    )
    .unwrap()
}

pub fn test_client_query_param(base_url: &str) -> BugzillaClient {
    BugzillaClient::new(
        base_url,
        "test-key",
        AuthMethod::QueryParam,
        ApiMode::Rest,
        None,
        &crate::tls::TlsConfig::default(),
    )
    .unwrap()
}

pub fn test_client_xmlrpc(base_url: &str) -> BugzillaClient {
    BugzillaClient::new(
        base_url,
        "test-key",
        AuthMethod::Header,
        ApiMode::XmlRpc,
        None,
        &crate::tls::TlsConfig::default(),
    )
    .unwrap()
}
