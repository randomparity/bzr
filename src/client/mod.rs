mod attachments;
mod bugs;
mod classifications;
mod comments;
mod components;
mod fields;
mod groups;
mod products;
mod server;
mod users;

use reqwest::header::HeaderValue;
use reqwest::RequestBuilder;
use serde::Deserialize;

use crate::error::{BzrError, Result};
use crate::http::{build_http_client, AUTH_HEADER_NAME, AUTH_QUERY_PARAM};
use crate::types::{ApiMode, AuthMethod};
use crate::xmlrpc::client::XmlRpcClient;

pub(super) fn encode_path(segment: &str) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    utf8_percent_encode(segment, NON_ALPHANUMERIC).to_string()
}

enum AuthConfig {
    Header(HeaderValue),
    QueryParam(String),
}

pub struct BugzillaClient {
    pub(super) http: reqwest::Client,
    pub(super) base_url: String,
    auth: AuthConfig,
    pub(super) api_key: String,
    pub(super) api_mode: ApiMode,
    pub(super) xmlrpc: Option<XmlRpcClient>,
}

/// Generic response for endpoints that return a single `id` field.
/// Used by bug creation, comment creation, product/component/user/group creation.
#[derive(Deserialize)]
pub(super) struct IdResponse {
    pub id: u64,
}

#[derive(Deserialize)]
struct ErrorResponse {
    #[serde(default)]
    error: bool,
    #[serde(default)]
    code: i64,
    #[serde(default)]
    message: Option<String>,
}

/// Bugzilla response keys that indicate real data is present alongside
/// an error payload. When any of these exist, the error is a non-fatal
/// server-side warning (e.g. from a Bugzilla extension) and the data
/// should be used.
const DATA_KEYS: &[&str] = &[
    "bugs",
    "comments",
    "attachments",
    "products",
    "groups",
    "users",
    "fields",
    "extensions",
    "classifications",
    "ids",
];

impl BugzillaClient {
    /// Check if a JSON object contains known Bugzilla data keys,
    /// indicating the response has real data alongside any error fields.
    fn has_data_fields(map: &serde_json::Map<String, serde_json::Value>) -> bool {
        DATA_KEYS.iter().any(|key| map.contains_key(*key))
    }

    pub fn new(
        base_url: &str,
        api_key: &str,
        auth_method: AuthMethod,
        api_mode: ApiMode,
    ) -> Result<Self> {
        let auth = match auth_method {
            AuthMethod::Header => {
                let value = HeaderValue::from_str(api_key)
                    .map_err(|_| BzrError::config("invalid API key characters"))?;
                AuthConfig::Header(value)
            }
            AuthMethod::QueryParam => AuthConfig::QueryParam(api_key.to_string()),
        };

        let http = build_http_client().map_err(BzrError::Http)?;

        let xmlrpc = match api_mode {
            ApiMode::Rest => None,
            ApiMode::XmlRpc | ApiMode::Hybrid => {
                Some(XmlRpcClient::new(http.clone(), base_url, api_key))
            }
        };

        tracing::debug!(base_url, %auth_method, %api_mode, "created Bugzilla client");

        Ok(BugzillaClient {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth,
            api_key: api_key.to_string(),
            api_mode,
            xmlrpc,
        })
    }

    pub(super) fn url(&self, path: &str) -> String {
        format!("{}/rest/{}", self.base_url, path.trim_start_matches('/'))
    }

    pub(super) fn xmlrpc_client(&self) -> Result<&XmlRpcClient> {
        self.xmlrpc.as_ref().ok_or_else(|| {
            BzrError::Config(
                "XML-RPC client not initialized — set api_mode to 'xmlrpc' or 'hybrid'".into(),
            )
        })
    }

    pub(super) fn apply_auth(&self, builder: RequestBuilder) -> RequestBuilder {
        match &self.auth {
            AuthConfig::Header(value) => builder.header(AUTH_HEADER_NAME, value.clone()),
            AuthConfig::QueryParam(key) => builder.query(&[(AUTH_QUERY_PARAM, key)]),
        }
    }

    pub(super) async fn send(&self, builder: RequestBuilder) -> Result<reqwest::Response> {
        let retry_builder = builder.try_clone();
        let resp = builder.send().await?;
        tracing::debug!(
            url = Self::safe_url(resp.url()),
            status = %resp.status(),
            "API response"
        );
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            if let Some(retried) = self.retry_with_alternate_auth(retry_builder).await? {
                return Ok(retried);
            }
        }
        self.check_error(resp).await
    }

    /// On 401, retry the request with the alternate auth method (header ↔ query param).
    /// Returns `Some(response)` if the retry succeeded, `None` if it failed or was not possible.
    async fn retry_with_alternate_auth(
        &self,
        retry_builder: Option<RequestBuilder>,
    ) -> Result<Option<reqwest::Response>> {
        let Some(clone) = retry_builder else {
            return Ok(None);
        };
        tracing::debug!("401 received, retrying with alternate auth method");
        let retried = self.apply_alternate_auth(clone)?.send().await?;
        tracing::debug!(
            url = Self::safe_url(retried.url()),
            status = %retried.status(),
            "auth fallback response"
        );
        if !retried.status().is_client_error() && !retried.status().is_server_error() {
            return Ok(Some(retried));
        }
        tracing::debug!("auth fallback also failed, returning original 401");
        Ok(None)
    }

    fn apply_alternate_auth(&self, builder: RequestBuilder) -> Result<RequestBuilder> {
        match &self.auth {
            AuthConfig::Header(_) => Ok(builder.query(&[(AUTH_QUERY_PARAM, &self.api_key)])),
            AuthConfig::QueryParam(_) => {
                let value = HeaderValue::from_str(&self.api_key).map_err(|e| {
                    BzrError::Config(format!("API key contains invalid header characters: {e}"))
                })?;
                Ok(builder.header(AUTH_HEADER_NAME, value))
            }
        }
    }

    fn safe_url(url: &reqwest::Url) -> String {
        format!("{}{}", url.origin().ascii_serialization(), url.path())
    }

    pub(super) async fn parse_json<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T> {
        let safe_url = Self::safe_url(resp.url());
        let body = resp.text().await?;

        tracing::trace!(
            url = safe_url,
            body = &body[..body.len().min(2048)],
            "response body"
        );

        // Parse once to serde_json::Value, then check for errors and
        // deserialize the target type from the same parsed value.
        let value: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            tracing::debug!(
                url = safe_url,
                error = %e,
                body_preview = &body[..body.len().min(512)],
                "JSON deserialization failed"
            );
            BzrError::Deserialize(format!("failed to parse response from {safe_url}: {e}"))
        })?;

        // Check for Bugzilla error responses that arrive with 200 status.
        // Some servers (e.g. IBM LTC Bugzilla) include error fields alongside
        // valid data — only treat the error as fatal when the response doesn't
        // also contain real data (indicated by common Bugzilla result keys).
        if let Some(map) = value.as_object() {
            let is_error = map.get("error").and_then(serde_json::Value::as_bool).unwrap_or(false);
            if is_error {
                let code = map.get("code").and_then(serde_json::Value::as_i64).unwrap_or(0);
                let message = map
                    .get("message")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let has_data = Self::has_data_fields(map);
                tracing::debug!(
                    url = safe_url,
                    code,
                    message = message.as_deref().unwrap_or("unknown"),
                    has_data,
                    "error payload in 200 response"
                );
                if !has_data {
                    return Err(BzrError::Api {
                        code,
                        message: message.unwrap_or_else(|| "unknown API error".into()),
                    });
                }
                tracing::warn!(
                    url = safe_url,
                    "server returned error alongside data; using data"
                );
            }
        }

        serde_json::from_value(value).map_err(|e| {
            BzrError::Deserialize(format!("failed to deserialize response from {safe_url}: {e}"))
        })
    }

    async fn check_error(&self, response: reqwest::Response) -> Result<reqwest::Response> {
        if response.status().is_client_error() || response.status().is_server_error() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::debug!(
                %status,
                body = &body[..body.len().min(512)],
                "API error response"
            );
            if let Ok(err) = serde_json::from_str::<ErrorResponse>(&body) {
                if err.error {
                    return Err(BzrError::Api {
                        code: err.code,
                        message: err.message.unwrap_or_else(|| status.to_string()),
                    });
                }
            }
            return Err(BzrError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }
        Ok(response)
    }
}

#[cfg(test)]
pub(super) mod test_helpers {
    use super::*;

    pub fn test_client(base_url: &str) -> BugzillaClient {
        BugzillaClient::new(base_url, "test-key", AuthMethod::Header, ApiMode::Rest).unwrap()
    }

    pub fn test_client_hybrid(base_url: &str) -> BugzillaClient {
        BugzillaClient::new(base_url, "test-key", AuthMethod::Header, ApiMode::Hybrid).unwrap()
    }

    pub fn test_client_query_param(base_url: &str) -> BugzillaClient {
        BugzillaClient::new(base_url, "test-key", AuthMethod::QueryParam, ApiMode::Rest).unwrap()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use test_helpers::{test_client, test_client_query_param};

    #[test]
    fn safe_url_strips_query_params() {
        let url = reqwest::Url::parse(&format!(
            "https://bugzilla.example.com/rest/bug/1?{}=secret",
            crate::http::AUTH_QUERY_PARAM
        ))
        .unwrap();
        let safe = BugzillaClient::safe_url(&url);
        assert!(
            !safe.contains("secret"),
            "API key should be stripped: {safe}"
        );
        assert!(
            safe.contains("/rest/bug/1"),
            "path should be preserved: {safe}"
        );
    }

    #[test]
    fn safe_url_preserves_path() {
        let url = reqwest::Url::parse("https://bugzilla.example.com/rest/bug/42").unwrap();
        let safe = BugzillaClient::safe_url(&url);
        assert_eq!(safe, "https://bugzilla.example.com/rest/bug/42");
    }

    #[tokio::test]
    async fn api_error_with_200_status() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/product"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 301,
                "message": "You are not authorized to access that product."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.get_product("Secret").await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("301"), "expected error code 301: {msg}");
        assert!(
            msg.contains("not authorized"),
            "expected auth error message: {msg}"
        );
    }

    #[tokio::test]
    async fn api_error_with_200_and_data_returns_data() {
        // Some servers (e.g. IBM LTC) return error fields alongside real
        // data. The data should be used and the error logged as a warning.
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 100_500,
                "message": "MirrorTool internal error",
                "bugs": [{"id": 42, "summary": "test bug", "status": "NEW"}]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let bug = client.get_bug("42", None, None).await.unwrap();
        assert_eq!(bug.id, 42);
        assert_eq!(bug.summary, "test bug");
    }

    #[tokio::test]
    async fn http_500_returns_error() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.search_users("anyone", false).await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("500") || msg.contains("Internal Server Error"),
            "expected 500 error: {msg}"
        );
    }

    #[tokio::test]
    async fn auth_fallback_header_to_query_param_on_401() {
        let mock = MockServer::start().await;
        // Success response requires query param auth (registered first)
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(query_param(crate::http::AUTH_QUERY_PARAM, "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [{"id": 1, "name": "alice@example.com"}]
            })))
            .expect(1)
            .mount(&mock)
            .await;
        // First request returns 401 (registered second, checked first by LIFO)
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": true,
                "code": 410,
                "message": "You must log in."
            })))
            .up_to_n_times(1)
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let users = client.search_users("alice", false).await.unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, "alice@example.com");
    }

    #[tokio::test]
    async fn auth_fallback_query_param_to_header_on_401() {
        let mock = MockServer::start().await;
        // Success response requires header auth (registered first)
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .and(wiremock::matchers::header(
                crate::http::AUTH_HEADER_NAME,
                "test-key",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [{"id": 2, "name": "bob@example.com"}]
            })))
            .expect(1)
            .mount(&mock)
            .await;
        // First request returns 401 (registered second, checked first by LIFO)
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": true,
                "code": 410,
                "message": "You must log in."
            })))
            .up_to_n_times(1)
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client_query_param(&mock.uri());
        let users = client.search_users("bob", false).await.unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, "bob@example.com");
    }

    #[tokio::test]
    async fn auth_fallback_both_fail_returns_original_error() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
                "error": true,
                "code": 410,
                "message": "You must log in."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.search_users("anyone", false).await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("410") || msg.contains("log in"),
            "expected auth error: {msg}"
        );
    }

    #[tokio::test]
    async fn non_401_errors_do_not_trigger_fallback() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/user"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": true,
                "code": 51,
                "message": "You are not authorized."
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.search_users("anyone", false).await.unwrap_err();
        assert!(err.to_string().contains("not authorized"));
    }
}
