use std::time::Duration;

use reqwest::header::HeaderValue;
use serde::Deserialize;

use crate::config::{AuthMethod, Config};
use crate::error::{BzrError, Result};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Deserialize)]
struct WhoAmIResponse {
    #[serde(default)]
    id: u64,
}

/// Returns the cached auth method for a server, or detects and persists it.
pub async fn resolve_auth_method(config: &mut Config, server_name: &str) -> Result<AuthMethod> {
    let srv = config
        .servers
        .get(server_name)
        .ok_or_else(|| BzrError::config(format!("server '{server_name}' not found in config")))?;

    if let Some(method) = srv.auth_method {
        return Ok(method);
    }

    let url = srv.url.clone();
    let api_key = srv.api_key.clone();

    let http = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| BzrError::Other(format!("failed to build HTTP client: {e}")))?;

    let method = detect_auth_method(&http, &url, &api_key).await?;

    tracing::info!(server = server_name, %method, "detected auth method");

    let srv_mut = config
        .servers
        .get_mut(server_name)
        .ok_or_else(|| BzrError::config(format!("server '{server_name}' not found in config")))?;
    srv_mut.auth_method = Some(method);
    config.save()?;

    Ok(method)
}

async fn detect_auth_method(
    http: &reqwest::Client,
    base_url: &str,
    api_key: &str,
) -> Result<AuthMethod> {
    let base = base_url.trim_end_matches('/');

    if !base.starts_with("https://") {
        tracing::warn!(
            url = base,
            "server URL is not HTTPS — API key will be sent in plaintext"
        );
    }

    let whoami_url = format!("{base}/rest/whoami");

    // Probe 1: header-based auth
    let key_val = HeaderValue::from_str(api_key)
        .map_err(|_| BzrError::config("invalid API key characters"))?;
    let resp = http
        .get(&whoami_url)
        .header("X-BUGZILLA-API-KEY", key_val)
        .send()
        .await?;

    if resp.status().is_success() {
        let body: WhoAmIResponse = resp.json().await?;
        if body.id > 0 {
            return Ok(AuthMethod::Header);
        }
    } else {
        tracing::debug!(status = %resp.status(), "header auth probe failed");
    }

    // Probe 2: query-param auth
    let resp = http
        .get(&whoami_url)
        .query(&[("Bugzilla_api_key", api_key)])
        .send()
        .await?;

    if resp.status().is_success() {
        let body: WhoAmIResponse = resp.json().await?;
        if body.id > 0 {
            return Ok(AuthMethod::QueryParam);
        }
    } else {
        tracing::debug!(status = %resp.status(), "query param auth probe failed");
    }

    Err(BzrError::Other(
        "auth detection failed: neither header nor query param auth \
         returned a valid user from rest/whoami. Check your API key \
         and server URL."
            .into(),
    ))
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn header_auth_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .and(header("X-BUGZILLA-API-KEY", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 42})))
            .mount(&server)
            .await;

        let result = detect_auth_method(&test_client(), &server.uri(), "test-key")
            .await
            .unwrap();
        assert_eq!(result, AuthMethod::Header);
    }

    #[tokio::test]
    async fn falls_back_to_query_param() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .and(header("X-BUGZILLA-API-KEY", "test-key"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .and(query_param("Bugzilla_api_key", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 7})))
            .mount(&server)
            .await;

        let result = detect_auth_method(&test_client(), &server.uri(), "test-key")
            .await
            .unwrap();
        assert_eq!(result, AuthMethod::QueryParam);
    }

    #[tokio::test]
    async fn both_methods_fail() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let result = detect_auth_method(&test_client(), &server.uri(), "test-key").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("auth detection failed"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn cached_method_skips_detection() {
        let mut config = Config::default();
        config.servers.insert(
            "cached".into(),
            crate::config::ServerConfig {
                url: "https://example.com".into(),
                api_key: "key".into(),
                auth_method: Some(AuthMethod::Header),
            },
        );

        let result = resolve_auth_method(&mut config, "cached").await.unwrap();
        assert_eq!(result, AuthMethod::Header);
    }

    #[tokio::test]
    async fn missing_server_returns_error() {
        let mut config = Config::default();
        let result = resolve_auth_method(&mut config, "nonexistent").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not found"), "unexpected error: {err}");
    }
}
