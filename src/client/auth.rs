use reqwest::header::HeaderValue;
use serde::Deserialize;

use crate::error::{BzrError, Result};
use crate::http::{build_http_client, AUTH_HEADER_NAME, AUTH_QUERY_PARAM};
use crate::types::{ApiMode, AuthMethod};

#[derive(Deserialize)]
struct WhoamiProbe {
    #[serde(default)]
    id: u64,
}

/// Result of server settings detection -- auth method, API mode, and
/// optionally the server version string. Returned by [`detect_server_settings`]
/// for the caller to persist as appropriate.
#[derive(Debug, Clone)]
pub struct DetectedServerSettings {
    pub auth_method: AuthMethod,
    pub api_mode: ApiMode,
    /// `Some` when the version endpoint responded successfully; `None` on
    /// transient failures. Callers should only persist `api_mode` and
    /// `server_version` when this is `Some`.
    pub server_version: Option<String>,
}

/// Detect auth method, API mode, and server version via network probes.
///
/// This is a pure detection function -- it does not read or write any
/// configuration. The caller is responsible for caching and persisting
/// the returned [`DetectedServerSettings`].
pub async fn detect_server_settings(
    url: &str,
    api_key: &str,
    email: Option<&str>,
) -> Result<DetectedServerSettings> {
    let http = build_http_client().map_err(BzrError::Http)?;

    let method = detect_auth_method(&http, url, api_key, email).await?;
    let (version, api_mode) = detect_version_and_mode(&http, url, api_key, method).await;

    tracing::info!(
        %method,
        %api_mode,
        version = version.as_deref().unwrap_or("unknown"),
        "detected server settings"
    );

    Ok(DetectedServerSettings {
        auth_method: method,
        api_mode,
        server_version: version,
    })
}

/// Detect server version and determine API mode.
///
/// Calls `GET /rest/version` to get the Bugzilla version string, then
/// applies thresholds to determine the best API transport.
async fn detect_version_and_mode(
    http: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    auth_method: AuthMethod,
) -> (Option<String>, ApiMode) {
    #[derive(serde::Deserialize)]
    struct VersionResp {
        version: String,
    }

    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/rest/version");

    let mut req = http.get(&url);
    match auth_method {
        AuthMethod::Header => {
            if let Ok(val) = reqwest::header::HeaderValue::from_str(api_key) {
                req = req.header(AUTH_HEADER_NAME, val);
            } else {
                tracing::debug!(
                    "API key has invalid header characters, sending version request without auth"
                );
            }
        }
        AuthMethod::QueryParam => {
            req = req.query(&[(AUTH_QUERY_PARAM, api_key)]);
        }
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("version detection failed (falling back to xmlrpc): {e}");
            return (None, ApiMode::XmlRpc);
        }
    };

    if !resp.status().is_success() {
        tracing::debug!(
            status = %resp.status(),
            "version endpoint not available, assuming pre-5.0"
        );
        return (None, ApiMode::XmlRpc);
    }

    let Ok(body) = resp.text().await else {
        tracing::warn!("version response body unreadable, falling back to xmlrpc");
        return (None, ApiMode::XmlRpc);
    };

    let Ok(parsed) = serde_json::from_str::<VersionResp>(&body) else {
        // Endpoint exists (200 OK) but returns non-standard body -- assume a
        // modern server with a custom extension; default to Hybrid.
        return (None, ApiMode::Hybrid);
    };

    let mode = version_to_api_mode(&parsed.version);
    tracing::debug!(version = %parsed.version, %mode, "determined API mode from version");
    (Some(parsed.version), mode)
}

/// Parse a Bugzilla version string and determine the API mode.
///
/// Version strings can be like "5.0.4", "5.1.2", "5.0.4.rh103", etc.
/// We extract major.minor and apply:
///   < 5.0 -> xmlrpc
///   >= 5.0, < 5.1 -> hybrid
///   >= 5.1 -> rest
fn version_to_api_mode(version: &str) -> ApiMode {
    let parts: Vec<&str> = version.split('.').collect();
    let major = parts.first().and_then(|s| s.parse::<u32>().ok());
    let minor = parts.get(1).and_then(|s| s.parse::<u32>().ok());

    match (major, minor) {
        (Some(major), _) if major < 5 => ApiMode::XmlRpc,
        (Some(5), Some(minor)) if minor < 1 => ApiMode::Hybrid,
        (Some(5), None) => ApiMode::Hybrid,
        (Some(_), _) => ApiMode::Rest,
        _ => ApiMode::Hybrid,
    }
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
            "server URL is not HTTPS -- API key will be sent in plaintext"
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
            // Prefer header when both work -- it avoids leaking keys in URLs.
            if method == AuthMethod::QueryParam
                && verify_header_auth_via_rest(http, base, &key_val).await
            {
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
    Err(BzrError::Auth(hint.into()))
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
    let header_req = http.get(&url).header(AUTH_HEADER_NAME, key_header.clone());
    let outcome = probe_whoami(header_req, AuthMethod::Header).await;
    match outcome {
        WhoamiOutcome::AuthRejected => {} // try query-param next
        other => return other,
    }

    // Probe: query-param auth
    let query_req = http.get(&url).query(&[(AUTH_QUERY_PARAM, api_key)]);
    probe_whoami(query_req, AuthMethod::QueryParam).await
}

async fn probe_whoami(request: reqwest::RequestBuilder, method: AuthMethod) -> WhoamiOutcome {
    let resp = match request.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("whoami {method} request failed: {e:#}");
            return WhoamiOutcome::NetworkError;
        }
    };

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    tracing::trace!(probe = "whoami", %method, %status, body, "auth probe response");
    if status.is_success() {
        // A valid whoami response contains a positive user ID. Treat id==0
        // (unauthenticated/anonymous) and unparseable bodies as auth failures
        // -- the server responded but didn't confirm our credentials.
        if let Ok(parsed) = serde_json::from_str::<WhoamiProbe>(&body) {
            if parsed.id > 0 {
                return WhoamiOutcome::Authenticated(method);
            }
        }
    } else if status == reqwest::StatusCode::NOT_FOUND {
        tracing::debug!("rest/whoami not available on this server");
        return WhoamiOutcome::NotFound;
    } else {
        tracing::debug!(%status, %method, "whoami auth probe failed");
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
#[serde(try_from = "serde_json::Value")]
struct ValidLoginResult(bool);

impl TryFrom<serde_json::Value> for ValidLoginResult {
    type Error = String;

    fn try_from(v: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        match v {
            serde_json::Value::Bool(b) => Ok(Self(b)),
            serde_json::Value::Number(n) => Ok(Self(n.as_u64() == Some(1))),
            other => Err(format!("expected bool or integer, got {other}")),
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
        &[("login", login), (AUTH_QUERY_PARAM, api_key)],
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
        req = req.header(AUTH_HEADER_NAME, hdr.clone());
    }
    let resp = req.send().await.ok()?;
    let status = resp.status();
    if !status.is_success() {
        tracing::debug!(%status, %method, "valid_login probe failed");
        return None;
    }
    let body_text = resp.text().await.ok()?;
    tracing::trace!(probe = "valid_login", %method, body = body_text, "auth probe response");
    let parsed: ValidLoginResponse = serde_json::from_str(&body_text).ok()?;
    if parsed.result.0 {
        Some(method)
    } else {
        tracing::debug!(%method, "valid_login returned false");
        None
    }
}

/// Try header auth on a real API endpoint to verify it works.
///
/// Some servers (e.g. IBM LTC Bugzilla) report header auth as unsupported
/// via `valid_login` but accept it on actual API endpoints. A minimal
/// `rest/bug?limit=1` request is used -- any 2xx confirms header auth works.
async fn verify_header_auth_via_rest(
    http: &reqwest::Client,
    base: &str,
    key_header: &HeaderValue,
) -> bool {
    let url = format!("{base}/rest/bug");
    let resp = http
        .get(&url)
        .query(&[("limit", "1")])
        .header(AUTH_HEADER_NAME, key_header.clone())
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
        crate::http::build_http_client().unwrap()
    }

    #[tokio::test]
    async fn header_auth_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .and(header(AUTH_HEADER_NAME, "test-key"))
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
            .and(header(AUTH_HEADER_NAME, "test-key"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .and(query_param(AUTH_QUERY_PARAM, "test-key"))
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
            .and(header(AUTH_HEADER_NAME, "test-key"))
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
            .and(header(AUTH_HEADER_NAME, "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": false})),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/valid_login"))
            .and(query_param("login", "user@example.com"))
            .and(query_param(AUTH_QUERY_PARAM, "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})),
            )
            .mount(&server)
            .await;

        // Header auth works on real API endpoints
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(header(AUTH_HEADER_NAME, "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})),
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
            .and(header(AUTH_HEADER_NAME, "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": false})),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/rest/valid_login"))
            .and(query_param("login", "user@example.com"))
            .and(query_param(AUTH_QUERY_PARAM, "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})),
            )
            .mount(&server)
            .await;

        // Header auth rejected on real API endpoints
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(header(AUTH_HEADER_NAME, "test-key"))
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
        // rather than failing -- header is the safest default.
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
            .and(header(AUTH_HEADER_NAME, "test-key"))
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
            &test_client(),
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
            &test_client(),
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
            &test_client(),
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
            &test_client(),
            "https://127.0.0.1:1",
            "test-key",
            AuthMethod::Header,
        )
        .await;
        assert!(version.is_none());
        assert_eq!(mode, ApiMode::XmlRpc);
    }

    /// Exercises detect_server_settings: probes auth and version without Config.
    #[tokio::test]
    async fn detect_server_settings_returns_all_fields() {
        let server = MockServer::start().await;

        // whoami succeeds with header auth
        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
            .mount(&server)
            .await;

        // version endpoint returns 5.1.2 -> REST mode
        Mock::given(method("GET"))
            .and(path("/rest/version"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
            )
            .mount(&server)
            .await;

        let detected = detect_server_settings(&server.uri(), "test-key", None)
            .await
            .unwrap();
        assert_eq!(detected.auth_method, AuthMethod::Header);
        assert_eq!(detected.api_mode, ApiMode::Rest);
        assert_eq!(detected.server_version.as_deref(), Some("5.1.2"));
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
            &test_client(),
            &server.uri(),
            "test-key",
            AuthMethod::Header,
        )
        .await;
        assert!(version.is_none());
        assert_eq!(mode, ApiMode::Hybrid);
    }
}
