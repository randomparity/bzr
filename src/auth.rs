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
    let email = srv.email.clone();

    let http = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| BzrError::Other(format!("failed to build HTTP client: {e}")))?;

    let method = detect_auth_method(&http, &url, &api_key, email.as_deref()).await?;

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
    email: Option<&str>,
) -> Result<AuthMethod> {
    let base = base_url.trim_end_matches('/');

    if !base.starts_with("https://") {
        tracing::warn!(
            url = base,
            "server URL is not HTTPS — API key will be sent in plaintext"
        );
    }

    let key_val = HeaderValue::from_str(api_key)
        .map_err(|_| BzrError::config("invalid API key characters"))?;

    // Try whoami endpoint first (Bugzilla 5.1+)
    let whoami = try_whoami(http, base, api_key, &key_val).await;
    match whoami {
        WhoamiOutcome::Authenticated(method) => return Ok(method),
        WhoamiOutcome::NotFound => {
            tracing::info!("falling back to rest/valid_login for older Bugzilla");
        }
        WhoamiOutcome::NetworkError => {
            tracing::warn!(
                "could not reach server during auth detection; \
                 defaulting to header auth"
            );
            return Ok(AuthMethod::Header);
        }
        WhoamiOutcome::AuthRejected => {}
    }

    // Fall back to valid_login endpoint (Bugzilla 5.0+, requires email)
    if let Some(login) = email {
        if let Some(method) = try_valid_login(http, base, api_key, &key_val, login).await {
            // valid_login can give false negatives for header auth on servers
            // with custom extensions (e.g. IBM LTC). When query_param is
            // detected, verify by probing a real endpoint with header auth.
            // Prefer header when both work — it avoids leaking keys in URLs.
            if method == AuthMethod::QueryParam && probe_header_on_api(http, base, &key_val).await {
                tracing::info!(
                    "header auth works on API endpoints despite valid_login \
                     rejecting it; preferring header"
                );
                return Ok(AuthMethod::Header);
            }
            return Ok(method);
        }
    }

    let hint = match whoami {
        WhoamiOutcome::NotFound if email.is_none() => {
            "auth detection failed: rest/whoami not available and no \
             --email provided for rest/valid_login fallback. \
             Re-run `bzr config set-server` with --email your@email."
        }
        WhoamiOutcome::NotFound => {
            "auth detection failed: rest/valid_login did not confirm \
             your credentials. Check your API key and email address."
        }
        _ => {
            "auth detection failed: could not authenticate with the \
             server. Check your API key and server URL."
        }
    };
    Err(BzrError::Other(hint.into()))
}

enum WhoamiOutcome {
    Authenticated(AuthMethod),
    NotFound,
    /// Server responded but auth was rejected (e.g. 401).
    AuthRejected,
    /// Could not reach the server at all (network/TLS/timeout).
    NetworkError,
}

async fn try_whoami(
    http: &reqwest::Client,
    base: &str,
    api_key: &str,
    key_header: &HeaderValue,
) -> WhoamiOutcome {
    let url = format!("{base}/rest/whoami");

    // Probe: header-based auth
    let resp = match http
        .get(&url)
        .header("X-BUGZILLA-API-KEY", key_header.clone())
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("whoami request failed: {e:#}");
            return WhoamiOutcome::NetworkError;
        }
    };

    if resp.status().is_success() {
        if let Ok(body) = resp.json::<WhoAmIResponse>().await {
            if body.id > 0 {
                return WhoamiOutcome::Authenticated(AuthMethod::Header);
            }
        }
    } else if resp.status() == reqwest::StatusCode::NOT_FOUND {
        tracing::debug!("rest/whoami not available on this server");
        return WhoamiOutcome::NotFound;
    } else {
        tracing::debug!(status = %resp.status(), "header auth probe failed");
    }

    // Probe: query-param auth
    let resp = match http
        .get(&url)
        .query(&[("Bugzilla_api_key", api_key)])
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("whoami query-param request failed: {e:#}");
            return WhoamiOutcome::NetworkError;
        }
    };

    if resp.status().is_success() {
        if let Ok(body) = resp.json::<WhoAmIResponse>().await {
            if body.id > 0 {
                return WhoamiOutcome::Authenticated(AuthMethod::QueryParam);
            }
        }
    } else if resp.status() == reqwest::StatusCode::NOT_FOUND {
        tracing::debug!("rest/whoami not available on this server");
        return WhoamiOutcome::NotFound;
    } else {
        tracing::debug!(status = %resp.status(), "query param auth probe failed");
    }

    WhoamiOutcome::AuthRejected
}

#[derive(Deserialize)]
struct ValidLoginResponse {
    #[serde(default)]
    result: ValidLoginResult,
}

/// Bugzilla returns `{"result": true}` (bool) or `{"result": 1}` (integer)
/// depending on version. Accept both.
#[derive(Deserialize, Default)]
#[serde(from = "serde_json::Value")]
struct ValidLoginResult(bool);

impl From<serde_json::Value> for ValidLoginResult {
    fn from(v: serde_json::Value) -> Self {
        match v {
            serde_json::Value::Bool(b) => Self(b),
            serde_json::Value::Number(n) => Self(n.as_u64() == Some(1)),
            _ => Self(false),
        }
    }
}

async fn try_valid_login(
    http: &reqwest::Client,
    base: &str,
    api_key: &str,
    key_header: &HeaderValue,
    login: &str,
) -> Option<AuthMethod> {
    let url = format!("{base}/rest/valid_login");

    // Probe: header-based auth
    if let Some(method) = probe_valid_login(
        http,
        &url,
        &[("login", login)],
        Some(key_header),
        AuthMethod::Header,
    )
    .await
    {
        return Some(method);
    }

    // Probe: query-param auth
    if let Some(method) = probe_valid_login(
        http,
        &url,
        &[("login", login), ("Bugzilla_api_key", api_key)],
        None,
        AuthMethod::QueryParam,
    )
    .await
    {
        return Some(method);
    }

    tracing::debug!("valid_login probes both failed");
    None
}

async fn probe_valid_login(
    http: &reqwest::Client,
    url: &str,
    query: &[(&str, &str)],
    key_header: Option<&HeaderValue>,
    method: AuthMethod,
) -> Option<AuthMethod> {
    let mut req = http.get(url).query(query);
    if let Some(hdr) = key_header {
        req = req.header("X-BUGZILLA-API-KEY", hdr.clone());
    }
    let resp = req.send().await.ok()?;
    if !resp.status().is_success() {
        tracing::debug!(status = %resp.status(), %method, "valid_login probe failed");
        return None;
    }
    let body: ValidLoginResponse = resp.json().await.ok()?;
    if body.result.0 {
        Some(method)
    } else {
        None
    }
}

/// Try header auth on a real API endpoint to verify it works.
///
/// Some servers (e.g. IBM LTC Bugzilla) report header auth as unsupported
/// via `valid_login` but accept it on actual API endpoints. A minimal
/// `rest/bug?limit=1` request is used — any 2xx confirms header auth works.
async fn probe_header_on_api(http: &reqwest::Client, base: &str, key_header: &HeaderValue) -> bool {
    let url = format!("{base}/rest/bug");
    let resp = http
        .get(&url)
        .query(&[("limit", "1")])
        .header("X-BUGZILLA-API-KEY", key_header.clone())
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => {
            tracing::debug!("header auth probe on rest/bug succeeded");
            true
        }
        Ok(r) => {
            tracing::debug!(
                status = %r.status(),
                "header auth probe on rest/bug failed"
            );
            false
        }
        Err(e) => {
            tracing::debug!("header auth probe request failed: {e}");
            false
        }
    }
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

        let result = detect_auth_method(&test_client(), &server.uri(), "test-key", None)
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

        let result = detect_auth_method(&test_client(), &server.uri(), "test-key", None)
            .await
            .unwrap();
        assert_eq!(result, AuthMethod::QueryParam);
    }

    #[tokio::test]
    async fn whoami_404_falls_back_to_valid_login_header() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/valid_login"))
            .and(query_param("login", "user@example.com"))
            .and(header("X-BUGZILLA-API-KEY", "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})),
            )
            .mount(&server)
            .await;

        let result = detect_auth_method(
            &test_client(),
            &server.uri(),
            "test-key",
            Some("user@example.com"),
        )
        .await
        .unwrap();
        assert_eq!(result, AuthMethod::Header);
    }

    #[tokio::test]
    async fn valid_login_query_param_but_header_works_on_api() {
        // Simulates IBM LTC-style servers: valid_login rejects header auth
        // but actual API endpoints accept it. Should prefer header.
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/valid_login"))
            .and(header("X-BUGZILLA-API-KEY", "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": false})),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/valid_login"))
            .and(query_param("login", "user@example.com"))
            .and(query_param("Bugzilla_api_key", "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})),
            )
            .mount(&server)
            .await;

        // Header auth works on real API endpoints
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(header("X-BUGZILLA-API-KEY", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
            .mount(&server)
            .await;

        let result = detect_auth_method(
            &test_client(),
            &server.uri(),
            "test-key",
            Some("user@example.com"),
        )
        .await
        .unwrap();
        assert_eq!(result, AuthMethod::Header);
    }

    #[tokio::test]
    async fn valid_login_query_param_and_header_fails_on_api() {
        // Server truly only supports query_param: valid_login and API
        // endpoints both reject header auth.
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/valid_login"))
            .and(header("X-BUGZILLA-API-KEY", "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": false})),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/valid_login"))
            .and(query_param("login", "user@example.com"))
            .and(query_param("Bugzilla_api_key", "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})),
            )
            .mount(&server)
            .await;

        // Header auth rejected on real API endpoints
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(header("X-BUGZILLA-API-KEY", "test-key"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let result = detect_auth_method(
            &test_client(),
            &server.uri(),
            "test-key",
            Some("user@example.com"),
        )
        .await
        .unwrap();
        assert_eq!(result, AuthMethod::QueryParam);
    }

    #[tokio::test]
    async fn whoami_401_no_email_gives_api_key_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let result = detect_auth_method(&test_client(), &server.uri(), "test-key", None).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Check your API key"),
            "should mention API key, got: {err}"
        );
    }

    #[tokio::test]
    async fn network_error_defaults_to_header() {
        // When the server is unreachable, default to header auth
        // rather than failing — header is the safest default.
        let result =
            detect_auth_method(&test_client(), "https://127.0.0.1:1", "test-key", None).await;
        assert!(result.is_ok(), "should default to header, got: {result:?}");
        assert_eq!(result.unwrap(), AuthMethod::Header);
    }

    #[tokio::test]
    async fn whoami_404_no_email_suggests_email_flag() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let result = detect_auth_method(&test_client(), &server.uri(), "test-key", None).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("--email"),
            "should suggest --email flag, got: {err}"
        );
    }

    #[tokio::test]
    async fn valid_login_accepts_integer_result() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/valid_login"))
            .and(query_param("login", "user@example.com"))
            .and(header("X-BUGZILLA-API-KEY", "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": 1})),
            )
            .mount(&server)
            .await;

        let result = detect_auth_method(
            &test_client(),
            &server.uri(),
            "test-key",
            Some("user@example.com"),
        )
        .await
        .unwrap();
        assert_eq!(result, AuthMethod::Header);
    }

    #[tokio::test]
    async fn both_methods_fail_with_email() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/valid_login"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": false})),
            )
            .mount(&server)
            .await;

        let result = detect_auth_method(
            &test_client(),
            &server.uri(),
            "test-key",
            Some("bad@example.com"),
        )
        .await;
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
                email: None,
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
