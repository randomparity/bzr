//! Auth detection orchestrator — split into submodules because the two
//! probing strategies (`whoami` for Bugzilla 5.1+, `valid_login` for 5.0+)
//! are each self-contained with their own types and logic.

mod valid_login;
mod whoami;

use reqwest::header::HeaderValue;

use crate::error::{BzrError, Result};
use crate::http::build_http_client;
use crate::types::{ApiMode, AuthMethod};

use self::valid_login::{detect_valid_login_auth, verify_header_auth_via_rest, ValidLoginOutcome};
use self::whoami::{detect_whoami_auth, WhoamiOutcome};

use super::version::detect_version_and_mode;

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
    let whoami = detect_whoami_auth(http, base, api_key, &key_val).await;
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
        match detect_valid_login_auth(http, base, api_key, &key_val, login).await {
            ValidLoginOutcome::Authenticated(method) => {
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
            ValidLoginOutcome::NetworkError => {
                tracing::warn!(
                    "valid_login probes failed due to network error; \
                     defaulting to header auth"
                );
                return Ok(AuthMethod::Header);
            }
            ValidLoginOutcome::AuthRejected => {}
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

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::client::test_helpers::test_http_client;
    use crate::http::AUTH_HEADER_NAME;

    #[tokio::test]
    async fn header_auth_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .and(header(AUTH_HEADER_NAME, "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 42})))
            .mount(&server)
            .await;

        let result = detect_auth_method(&test_http_client(), &server.uri(), "test-key", None)
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
            .and(query_param(crate::http::AUTH_QUERY_PARAM, "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 7})))
            .mount(&server)
            .await;

        let result = detect_auth_method(&test_http_client(), &server.uri(), "test-key", None)
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
            &test_http_client(),
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
            .and(query_param(crate::http::AUTH_QUERY_PARAM, "test-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"result": true})),
            )
            .mount(&server)
            .await;

        // Header auth works on real API endpoints
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(header(AUTH_HEADER_NAME, "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
            .mount(&server)
            .await;

        let result = detect_auth_method(
            &test_http_client(),
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
            .and(query_param(crate::http::AUTH_QUERY_PARAM, "test-key"))
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
            &test_http_client(),
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

        let result = detect_auth_method(&test_http_client(), &server.uri(), "test-key", None).await;
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
            detect_auth_method(&test_http_client(), "https://127.0.0.1:1", "test-key", None).await;
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

        let result = detect_auth_method(&test_http_client(), &server.uri(), "test-key", None).await;
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
            &test_http_client(),
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
            &test_http_client(),
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
}
