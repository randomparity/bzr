use crate::http::apply_auth;
use crate::types::{ApiMode, AuthMethod};

/// Detect server version and determine API mode.
///
/// Calls `GET /rest/version` to get the Bugzilla version string, then
/// applies thresholds to determine the best API transport.
pub(super) async fn detect_version_and_mode(
    http: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    auth_method: AuthMethod,
) -> (Option<String>, ApiMode) {
    #[derive(serde::Deserialize)]
    struct VersionResponse {
        version: String,
    }

    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/rest/version");

    let req = match apply_auth(http.get(&url), api_key, auth_method) {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("auth setup failed for version probe: {e}");
            // Fall back to unauthenticated request — version endpoint is often public.
            http.get(&url)
        }
    };

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

    let Ok(parsed) = serde_json::from_str::<VersionResponse>(&body) else {
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

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::client::test_helpers::test_http_client;

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
            &test_http_client(),
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
            &test_http_client(),
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
            &test_http_client(),
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
            &test_http_client(),
            "https://127.0.0.1:1",
            "test-key",
            AuthMethod::Header,
        )
        .await;
        assert!(version.is_none());
        assert_eq!(mode, ApiMode::XmlRpc);
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
            &test_http_client(),
            &server.uri(),
            "test-key",
            AuthMethod::Header,
        )
        .await;
        assert!(version.is_none());
        assert_eq!(mode, ApiMode::Hybrid);
    }
}
