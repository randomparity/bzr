mod attachment;
pub(crate) mod auth;
pub(crate) use auth::{detect_server_settings, DetectedServerSettings};
mod bug;
mod classification;
mod comment;
mod component;
mod field;
pub(crate) use field::FIELD_ALIASES;
mod group;
mod product;
mod server;
mod user;
mod version;

use reqwest::header::HeaderValue;
use reqwest::RequestBuilder;
use serde::Deserialize;

use crate::error::{BzrError, Result};
use crate::http::{AUTH_HEADER_NAME, AUTH_QUERY_PARAM};
use crate::types::BugzillaUser;
use crate::types::{ApiMode, AuthMethod};
use crate::xmlrpc::client::XmlRpcClient;

/// Default fields for user queries (basic info).
pub(super) const USER_FIELDS_BASIC: &str = "id,name,real_name,email,groups";
/// Extended fields for detailed user queries.
pub(super) const USER_FIELDS_DETAILED: &str = "id,name,real_name,email,can_login,groups";

#[derive(Deserialize)]
pub(super) struct UserSearchResponse {
    pub(super) users: Vec<BugzillaUser>,
}

pub(super) fn encode_path(segment: &str) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    utf8_percent_encode(segment, NON_ALPHANUMERIC).to_string()
}

enum PreparedAuth {
    Header(HeaderValue),
    QueryParam(String),
}

/// HTTP client for the Bugzilla REST API.
///
/// Update methods use the identifier type that the Bugzilla REST API accepts:
/// - `u64` for resources identified only by numeric ID (e.g. `update_component`)
/// - `&str` for resources that accept name-based addressing (e.g. `update_product`, `update_user`)
pub struct BugzillaClient {
    pub(super) http: reqwest::Client,
    pub(super) base_url: String,
    auth: PreparedAuth,
    pub(super) api_key: String,
    pub(super) api_mode: ApiMode,
    pub(super) xmlrpc: Option<XmlRpcClient>,
    /// Email hint for Bugzilla 5.0 compatibility (whoami fallback via user lookup).
    email_hint: Option<String>,
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
    #[serde(default, deserialize_with = "deserialize_code")]
    code: i64,
    #[serde(default)]
    message: Option<String>,
}

/// Bugzilla returns error codes as integers on some versions and as
/// strings on others (e.g. `"32610"` on Bugzilla 5.3). Accept both.
fn deserialize_code<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<i64, D::Error> {
    use serde::de;

    struct CodeVisitor;

    impl de::Visitor<'_> for CodeVisitor {
        type Value = i64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an integer or string-encoded integer")
        }

        fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<i64, E> {
            Ok(v)
        }

        fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<i64, E> {
            i64::try_from(v).map_err(E::custom)
        }

        fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<i64, E> {
            v.parse::<i64>().map_err(E::custom)
        }
    }

    deserializer.deserialize_any(CodeVisitor)
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

/// A candidate envelope extractor: the key to look for in the top-level
/// JSON object and a function that receives the full `Value` and returns
/// the typed result. Used by [`BugzillaClient::try_envelopes`].
type EnvelopeCandidate<T> = (&'static str, fn(serde_json::Value) -> Result<T>);

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
        email_hint: Option<&str>,
        tls_config: &crate::tls::TlsConfig,
    ) -> Result<Self> {
        let auth = match auth_method {
            AuthMethod::Header => {
                let value = HeaderValue::from_str(api_key)
                    .map_err(|_| BzrError::config("invalid API key characters"))?;
                PreparedAuth::Header(value)
            }
            AuthMethod::QueryParam => PreparedAuth::QueryParam(api_key.to_string()),
        };

        let http = crate::tls::build_tls_client(tls_config)?;

        // Always construct the XML-RPC client — even in REST mode, some
        // methods (e.g. Group.get on Bugzilla 5.3+) require XML-RPC fallback
        // because the REST endpoint is broken for them.
        if api_mode != ApiMode::Rest && auth_method == AuthMethod::Header {
            tracing::info!(
                "XML-RPC always sends API key in request body, \
                 overriding configured header auth for XML-RPC calls"
            );
        }
        let xmlrpc = Some(XmlRpcClient::new(http.clone(), base_url, api_key));

        tracing::debug!(base_url, %auth_method, %api_mode, "created Bugzilla client");

        Ok(BugzillaClient {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth,
            api_key: api_key.to_string(),
            api_mode,
            xmlrpc,
            email_hint: email_hint.map(String::from),
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

    /// Send a GET request and deserialize the JSON response.
    pub(super) async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let req = self.apply_auth(self.http.get(self.url(path)));
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }

    /// Send a GET request with query parameters and deserialize the JSON response.
    pub(super) async fn get_json_query<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<T> {
        let req = self.apply_auth(self.http.get(self.url(path)).query(query));
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }

    /// Send a POST request with a JSON body and return the created resource ID.
    pub(super) async fn post_json_id(
        &self,
        path: &str,
        body: &impl serde::Serialize,
    ) -> Result<u64> {
        let req = self.apply_auth(self.http.post(self.url(path)).json(body));
        let resp = self.send(req).await?;
        let data: IdResponse = self.parse_json(resp).await?;
        Ok(data.id)
    }

    /// Send a PUT request with a JSON body to a REST resource path.
    pub(super) async fn put_json(&self, path: &str, body: &impl serde::Serialize) -> Result<()> {
        let req = self.apply_auth(self.http.put(self.url(path)).json(body));
        self.send(req).await?;
        Ok(())
    }

    /// Send a PUT request and deserialize the JSON response.
    pub(super) async fn put_json_response<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &impl serde::Serialize,
    ) -> Result<T> {
        let req = self.apply_auth(self.http.put(self.url(path)).json(body));
        let resp = self.send(req).await?;
        self.parse_json(resp).await
    }

    /// Apply auth credentials to a request. Infallible because the API key
    /// was validated at client construction time. Delegates to the shared
    /// [`crate::http::apply_auth_to_request`] primitive.
    pub(super) fn apply_auth(&self, builder: RequestBuilder) -> RequestBuilder {
        match &self.auth {
            PreparedAuth::Header(value) => {
                crate::http::apply_auth_to_request(builder, Some(value), None)
            }
            PreparedAuth::QueryParam(key) => {
                crate::http::apply_auth_to_request(builder, None, Some(key))
            }
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
        self.check_response_status(resp).await
    }

    /// On 401, retry the request with the alternate auth method (header ↔ query param).
    /// Returns `Ok(Some(response))` if the retry succeeded, `Ok(None)` if the retry
    /// also failed or wasn't possible, or `Err` on transport-level failures.
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
        if retried.status().is_success() {
            return Ok(Some(retried));
        }
        tracing::debug!("auth fallback also failed, returning original 401");
        Ok(None)
    }

    fn apply_alternate_auth(&self, builder: RequestBuilder) -> Result<RequestBuilder> {
        match &self.auth {
            PreparedAuth::Header(_) => Ok(builder.query(&[(AUTH_QUERY_PARAM, &self.api_key)])),
            PreparedAuth::QueryParam(_) => {
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

        let value: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            tracing::debug!(
                url = safe_url,
                error = %e,
                body_preview = &body[..body.len().min(512)],
                "JSON deserialization failed"
            );
            BzrError::Deserialize(format!(
                "failed to parse response from {safe_url}: {e}\nbody preview ({} chars): {}",
                body.chars().count().min(BODY_PREVIEW_MAX_BYTES),
                format_body_preview(&body),
            ))
        })?;

        Self::check_bugzilla_200_error(&value, &safe_url)?;

        serde_json::from_value(value).map_err(|e| {
            BzrError::Deserialize(format!(
                "failed to deserialize response from {safe_url}: {e}\nbody preview ({} chars): {}",
                body.chars().count().min(BODY_PREVIEW_MAX_BYTES),
                format_body_preview(&body),
            ))
        })
    }

    /// Parse a response body to a generic [`serde_json::Value`].
    ///
    /// Performs the same body-read, JSON-parse, and 200-error check as
    /// [`Self::parse_json`], but stops short of typed deserialization.
    /// Used by callers that need to inspect the response envelope before
    /// committing to a concrete type (see [`Self::try_envelopes`]).
    pub(super) async fn parse_json_value(
        &self,
        resp: reqwest::Response,
    ) -> Result<serde_json::Value> {
        let safe_url = Self::safe_url(resp.url());
        let body = resp.text().await?;

        tracing::trace!(
            url = safe_url,
            body = &body[..body.len().min(2048)],
            "response body"
        );

        let value: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            tracing::debug!(
                url = safe_url,
                error = %e,
                body_preview = &body[..body.len().min(512)],
                "JSON deserialization failed"
            );
            BzrError::Deserialize(format!(
                "failed to parse response from {safe_url}: {e}\nbody preview ({} chars): {}",
                body.chars().count().min(BODY_PREVIEW_MAX_BYTES),
                format_body_preview(&body),
            ))
        })?;

        Self::check_bugzilla_200_error(&value, &safe_url)?;
        Ok(value)
    }

    /// Send a GET request and return the parsed JSON body as a `Value`.
    pub(super) async fn get_json_value(&self, path: &str) -> Result<serde_json::Value> {
        let req = self.apply_auth(self.http.get(self.url(path)));
        let resp = self.send(req).await?;
        self.parse_json_value(resp).await
    }

    /// Detect Bugzilla error payloads that arrive with HTTP 200 status.
    ///
    /// Some servers (e.g. IBM LTC Bugzilla) include error fields alongside
    /// valid data — only treat the error as fatal when the response doesn't
    /// also contain real data (indicated by common Bugzilla result keys).
    fn check_bugzilla_200_error(value: &serde_json::Value, url: &str) -> Result<()> {
        let Some(map) = value.as_object() else {
            return Ok(());
        };
        let is_error = map
            .get("error")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        if !is_error {
            return Ok(());
        }

        let code = map
            .get("code")
            .and_then(|v| {
                v.as_i64()
                    .or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))
            })
            .unwrap_or(-1);
        let message = map
            .get("message")
            .and_then(|v| v.as_str())
            .map(String::from);
        let has_data = Self::has_data_fields(map);

        tracing::debug!(
            url,
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
        tracing::warn!(url, "server returned error alongside data; using data");
        Ok(())
    }

    /// Try each envelope shape in order, returning the first that succeeds.
    ///
    /// `candidates` is a slice of `(envelope_key, extractor)` pairs. The
    /// helper inspects the parsed JSON's top-level keys: it tries the
    /// extractors whose `envelope_key` is present first, then any
    /// remaining as fallbacks. On total failure, returns the first
    /// candidate's error annotated with the list of envelopes tried and
    /// a redacted body preview built from the re-serialized `Value`
    /// (so the user sees what shape the server actually sent).
    ///
    /// Used to tolerate response-shape variants between Bugzilla
    /// deployments — e.g. `bug/<id>/attachment` returns `bugs`-keyed on
    /// stock 5.0.x but `attachments`-keyed on some IBM-style deployments.
    pub(super) fn try_envelopes<T>(
        value: &serde_json::Value,
        candidates: &[EnvelopeCandidate<T>],
    ) -> Result<T> {
        let present_keys: std::collections::HashSet<&str> = value
            .as_object()
            .map(|m| m.keys().map(String::as_str).collect())
            .unwrap_or_default();

        let mut first_error: Option<BzrError> = None;

        // First pass: try candidates whose envelope key is present.
        for (key, extractor) in candidates {
            if present_keys.contains(*key) {
                match extractor(value.clone()) {
                    Ok(v) => return Ok(v),
                    Err(e) if first_error.is_none() => first_error = Some(e),
                    Err(_) => {}
                }
            }
        }

        // Second pass: try remaining candidates as fallbacks.
        for (key, extractor) in candidates {
            if !present_keys.contains(*key) {
                match extractor(value.clone()) {
                    Ok(v) => return Ok(v),
                    Err(e) if first_error.is_none() => first_error = Some(e),
                    Err(_) => {}
                }
            }
        }

        let envelope_list = candidates
            .iter()
            .map(|(k, _)| *k)
            .collect::<Vec<_>>()
            .join(", ");
        let underlying =
            first_error.map_or_else(|| "no candidates provided".to_string(), |e| e.to_string());
        // Re-serialize the parsed Value so the user can see what shape the
        // server actually sent. This is the most important diagnostic case
        // for issue #135 — a previously-unseen envelope shape.
        let body_str =
            serde_json::to_string(value).unwrap_or_else(|_| "<value not serializable>".to_string());
        let preview = format_body_preview(&body_str);
        let preview_chars = body_str.chars().count().min(BODY_PREVIEW_MAX_BYTES);
        Err(BzrError::Deserialize(format!(
            "no matching envelope (tried envelopes: {envelope_list}): {underlying}\nbody preview ({preview_chars} chars): {preview}"
        )))
    }

    async fn check_response_status(
        &self,
        response: reqwest::Response,
    ) -> Result<reqwest::Response> {
        if response.status().is_client_error() || response.status().is_server_error() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|e| {
                tracing::warn!("failed to read error response body: {e}");
                String::new()
            });
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

/// Maximum length of the body excerpt embedded in deserialization errors.
/// 512 bytes is enough to capture the top-level keys of any realistic
/// Bugzilla envelope while keeping the error message human-scaled.
const BODY_PREVIEW_MAX_BYTES: usize = 512;

/// Format a response body for inclusion in a `BzrError::Deserialize` message.
///
/// Truncates to [`BODY_PREVIEW_MAX_BYTES`] on a UTF-8 char boundary,
/// appends `…` when truncated, runs the result through
/// [`crate::http::redact_api_key`] to strip echoed-back API keys, and
/// collapses internal newlines and tabs to single spaces so the preview
/// stays on one line beneath the main error.
///
/// Called by `parse_json` when deserializing JSON fails.
fn format_body_preview(body: &str) -> String {
    let truncated_end = body
        .char_indices()
        .take_while(|(i, _)| *i < BODY_PREVIEW_MAX_BYTES)
        .last()
        .map_or(0, |(i, c)| i + c.len_utf8());

    let mut preview = String::with_capacity(truncated_end + 4);
    preview.push_str(&body[..truncated_end]);
    if truncated_end < body.len() {
        preview.push('…');
    }

    // Collapse whitespace so the preview stays on one line in error output.
    let collapsed: String = preview
        .chars()
        .map(|c| {
            if c == '\n' || c == '\t' || c == '\r' {
                ' '
            } else {
                c
            }
        })
        .collect();

    crate::http::redact_api_key(&collapsed)
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
pub(super) mod test_helpers {
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

    #[test]
    fn new_trims_trailing_slash_and_keeps_email_hint() {
        let client = BugzillaClient::new(
            "https://bugzilla.example.com/",
            "test-key",
            AuthMethod::Header,
            ApiMode::Rest,
            Some("user@example.com"),
            &crate::tls::TlsConfig::default(),
        )
        .unwrap();

        assert_eq!(client.base_url, "https://bugzilla.example.com");
        assert_eq!(client.email_hint.as_deref(), Some("user@example.com"));
    }

    #[test]
    fn apply_auth_adds_query_param_credentials() {
        let client = test_client_query_param("https://bugzilla.example.com");
        let request = client
            .apply_auth(client.http.get(client.url("bug")))
            .build()
            .unwrap();
        let expected_query = format!("{AUTH_QUERY_PARAM}=test-key");
        assert_eq!(request.url().query(), Some(expected_query.as_str()));
    }

    #[test]
    fn alternate_auth_rejects_invalid_header_characters() {
        let client = BugzillaClient::new(
            "https://bugzilla.example.com",
            "bad\nkey",
            AuthMethod::QueryParam,
            ApiMode::Rest,
            None,
            &crate::tls::TlsConfig::default(),
        )
        .unwrap();

        let builder = client.http.get(client.url("bug"));
        let err = client.apply_alternate_auth(builder).unwrap_err();
        assert!(err.to_string().contains("invalid header characters"));
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

    #[tokio::test]
    async fn api_error_with_string_code_parsed_correctly() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/group"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": true,
                "code": "32610",
                "message": "For security reasons, you must use HTTP POST."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let resp = client
            .http
            .get(format!("{}/rest/group", mock.uri()))
            .send()
            .await
            .unwrap();
        let err = client.check_response_status(resp).await.unwrap_err();
        assert!(
            matches!(&err, crate::error::BzrError::Api { code: 32610, .. }),
            "expected Api error with code 32610, got: {err}"
        );
    }

    #[test]
    fn error_response_parses_unsigned_integer_code() {
        let json = r#"{"error":true,"code":32610,"message":"x"}"#;
        let err: super::ErrorResponse = serde_json::from_str(json).unwrap();
        assert_eq!(err.code, 32610);
    }

    #[test]
    fn error_response_parses_negative_integer_code() {
        let json = r#"{"error":true,"code":-7,"message":"x"}"#;
        let err: super::ErrorResponse = serde_json::from_str(json).unwrap();
        assert_eq!(err.code, -7);
    }

    #[tokio::test]
    async fn api_200_error_without_code_field_uses_minus_one() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/group"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "message": "no code"
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client
            .get_json_query::<serde_json::Value>("group", &[])
            .await
            .unwrap_err();
        assert!(
            matches!(&err, crate::error::BzrError::Api { code: -1, .. }),
            "expected Api error with code -1, got: {err}"
        );
    }

    #[tokio::test]
    async fn api_200_error_with_string_code_parsed_correctly() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/group"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": "32610",
                "message": "For security reasons, you must use HTTP POST."
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err: crate::error::BzrError = client
            .get_json_query::<serde_json::Value>("group", &[])
            .await
            .unwrap_err();
        assert!(
            matches!(&err, crate::error::BzrError::Api { code: 32610, .. }),
            "expected Api error with code 32610, got: {err}"
        );
    }

    #[test]
    fn format_body_preview_returns_short_body_unchanged_in_content() {
        let body = r#"{"error":false,"attachments":[]}"#;
        let preview = super::format_body_preview(body);
        assert!(
            preview.contains(r#""attachments":[]"#),
            "should contain original JSON: {preview}"
        );
        assert!(
            !preview.ends_with('…'),
            "short body should not be truncated: {preview}"
        );
    }

    #[test]
    fn format_body_preview_truncates_long_body_with_ellipsis() {
        let body = "x".repeat(2048);
        let preview = super::format_body_preview(&body);
        assert!(
            preview.ends_with('…'),
            "long body should end with ellipsis: ...{}",
            &preview[preview.len().saturating_sub(20)..]
        );
        // Length check: 512 'x' chars + 1 ellipsis char (3 bytes UTF-8) = 515 bytes max for the content.
        assert!(
            preview.chars().count() <= 513,
            "preview should be <=513 chars (512 + ellipsis), got {}",
            preview.chars().count()
        );
    }

    #[test]
    fn format_body_preview_redacts_api_key_in_body() {
        let body = r#"{"echo":"http://h/rest/bug?Bugzilla_api_key=Sup3rSecret&x=1"}"#;
        let preview = super::format_body_preview(body);
        assert!(
            !preview.contains("Sup3rSecret"),
            "API key must be redacted: {preview}"
        );
        assert!(
            preview.contains("Bugzilla_api_key=[REDACTED]"),
            "redaction marker missing: {preview}"
        );
    }

    #[test]
    fn format_body_preview_collapses_internal_whitespace() {
        let body = "{\n  \"a\": 1,\n\t\"b\": 2\n}";
        let preview = super::format_body_preview(body);
        assert!(
            !preview.contains('\n'),
            "newlines should be collapsed: {preview:?}"
        );
        assert!(
            !preview.contains('\t'),
            "tabs should be collapsed: {preview:?}"
        );
    }

    #[test]
    fn format_body_preview_truncates_on_utf8_boundary() {
        // 200 ASCII chars + 200 multi-byte chars (3 bytes each, ☃ = U+2603) = 800 bytes total.
        // Truncation at 512 *bytes* must not split a multi-byte character.
        let mut body = "a".repeat(200);
        for _ in 0..200 {
            body.push('☃');
        }
        let preview = super::format_body_preview(&body);
        // If we sliced mid-codepoint, this would panic before reaching the assert.
        // Confirm the trailing ellipsis is intact (proves no panic and proves truncation occurred).
        assert!(preview.ends_with('…'), "expected truncation: {preview}");
    }

    #[test]
    fn format_body_preview_handles_empty_body() {
        let preview = super::format_body_preview("");
        assert_eq!(preview, "", "empty body should produce empty preview");
    }

    #[test]
    fn try_envelopes_returns_first_candidate_match() {
        let value = serde_json::json!({"bugs": {"42": [{"id": 1}]}});
        let extract_bugs: fn(serde_json::Value) -> Result<i32> = |_v| Ok(1);
        let extract_attachments: fn(serde_json::Value) -> Result<i32> = |_v| Ok(2);
        let result = super::BugzillaClient::try_envelopes(
            &value,
            &[("bugs", extract_bugs), ("attachments", extract_attachments)],
        )
        .unwrap();
        assert_eq!(
            result, 1,
            "should pick `bugs` extractor when `bugs` key is present"
        );
    }

    #[test]
    fn try_envelopes_falls_back_to_alt_envelope() {
        let value = serde_json::json!({"attachments": [{"id": 1}]});
        let extract_bugs: fn(serde_json::Value) -> Result<i32> = |_v| Ok(1);
        let extract_attachments: fn(serde_json::Value) -> Result<i32> = |_v| Ok(2);
        let result = super::BugzillaClient::try_envelopes(
            &value,
            &[("bugs", extract_bugs), ("attachments", extract_attachments)],
        )
        .unwrap();
        assert_eq!(
            result, 2,
            "should pick `attachments` extractor when only `attachments` key is present"
        );
    }

    #[test]
    fn try_envelopes_returns_first_error_when_no_candidate_matches() {
        let value = serde_json::json!({"unknown_key": "unknown_value"});
        let extract_bugs: fn(serde_json::Value) -> Result<i32> = |_v| {
            Err(crate::error::BzrError::Deserialize(
                "bugs extractor failed".into(),
            ))
        };
        let extract_attachments: fn(serde_json::Value) -> Result<i32> = |_v| {
            Err(crate::error::BzrError::Deserialize(
                "attachments extractor failed".into(),
            ))
        };
        let err = super::BugzillaClient::try_envelopes(
            &value,
            &[("bugs", extract_bugs), ("attachments", extract_attachments)],
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("tried envelopes"),
            "should mention attempted envelopes: {msg}"
        );
        assert!(msg.contains("bugs"), "should list 'bugs': {msg}");
        assert!(
            msg.contains("attachments"),
            "should list 'attachments': {msg}"
        );
        assert!(
            msg.contains("bugs extractor failed"),
            "should preserve first extractor's error: {msg}"
        );
        assert!(
            msg.contains("body preview"),
            "should include body preview: {msg}"
        );
        assert!(
            msg.contains("unknown_key"),
            "preview should contain Value contents: {msg}"
        );
    }

    #[test]
    fn try_envelopes_falls_through_when_keyed_extractor_errors() {
        // The `bugs` key is present but its value can't be extracted (wrong shape).
        // The fallback `attachments` extractor (no key required) should still run.
        let value = serde_json::json!({"bugs": "not_an_object", "attachments": [{"id": 1}]});
        let extract_bugs: fn(serde_json::Value) -> Result<i32> =
            |_v| Err(crate::error::BzrError::Deserialize("bad bugs shape".into()));
        let extract_attachments: fn(serde_json::Value) -> Result<i32> = |_v| Ok(2);
        let result = super::BugzillaClient::try_envelopes(
            &value,
            &[("bugs", extract_bugs), ("attachments", extract_attachments)],
        )
        .unwrap();
        assert_eq!(result, 2);
    }

    #[test]
    fn format_body_preview_handles_exactly_512_byte_body() {
        // A body whose length exactly equals the truncation threshold should
        // be returned in full with no ellipsis (off-by-one boundary check).
        let body = "a".repeat(512);
        let preview = super::format_body_preview(&body);
        assert_eq!(
            preview.chars().count(),
            512,
            "exact-512 body should not be truncated"
        );
        assert!(
            !preview.ends_with('…'),
            "exact-512 body should have no ellipsis: ...{}",
            &preview[preview.len().saturating_sub(20)..]
        );
    }

    #[tokio::test]
    async fn parse_json_includes_body_preview_on_typed_failure() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/attachment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                // Wrong shape — has neither `bugs` nor matches AttachmentBugResponse.
                "wrong_key": [1, 2, 3]
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.get_attachments(42).await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("body preview"),
            "error should include body preview: {msg}"
        );
        assert!(
            msg.contains("wrong_key"),
            "preview should contain offending JSON keys: {msg}"
        );
    }

    #[tokio::test]
    async fn parse_json_includes_body_preview_on_invalid_json() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/attachment"))
            .respond_with(ResponseTemplate::new(200).set_body_string("{not valid json"))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.get_attachments(42).await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("body preview"),
            "error should include body preview: {msg}"
        );
        assert!(
            msg.contains("not valid json"),
            "preview should contain raw body: {msg}"
        );
    }

    #[tokio::test]
    async fn get_json_value_returns_parsed_value_without_typed_check() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/anything"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "arbitrary_key": "arbitrary_value",
                "nested": {"inner": 42}
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let value = client.get_json_value("anything").await.unwrap();
        assert_eq!(value["arbitrary_key"], "arbitrary_value");
        assert_eq!(value["nested"]["inner"], 42);
    }

    #[tokio::test]
    async fn get_json_value_runs_check_bugzilla_200_error() {
        // A 200 response with `error: true` and no data fields must still
        // produce a BzrError::Api — get_json_value should run the same
        // 200-error check that get_json does.
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/anything"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 301,
                "message": "denied"
            })))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.get_json_value("anything").await.unwrap_err();
        assert!(
            matches!(err, crate::error::BzrError::Api { code: 301, .. }),
            "expected Api error, got: {err}"
        );
    }

    #[tokio::test]
    async fn parse_json_redacts_api_key_in_body_preview() {
        let mock = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/rest/bug/42/attachment"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"echo":"http://h/rest?Bugzilla_api_key=LeakedKey9","wrong":true}"#,
            ))
            .mount(&mock)
            .await;

        let client = test_client(&mock.uri());
        let err = client.get_attachments(42).await.unwrap_err();
        let msg = err.to_string();
        assert!(
            !msg.contains("LeakedKey9"),
            "API key must not appear in error: {msg}"
        );
        assert!(
            msg.contains("[REDACTED]"),
            "redaction marker should be present: {msg}"
        );
    }
}
