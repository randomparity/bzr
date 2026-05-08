mod attachment;
pub(crate) mod auth;
pub(crate) use auth::{detect_server_settings, DetectedServerSettings};
mod bug;
mod classification;
mod comment;
mod component;
mod field;
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

/// Configuration needed to construct a [`BugzillaClient`].
#[non_exhaustive]
#[derive(Clone, Copy)]
pub struct BugzillaClientConfig<'a> {
    pub base_url: &'a str,
    pub credential: &'a str,
    pub auth_method: AuthMethod,
    pub api_mode: ApiMode,
    pub email_hint: Option<&'a str>,
    pub tls_config: &'a crate::tls::TlsConfig,
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
/// JSON object and a function that receives a borrowed `Value` and returns
/// the typed result. Used by [`BugzillaClient::try_envelopes`].
type EnvelopeCandidate<T> = (&'static str, fn(&serde_json::Value) -> Result<T>);

impl BugzillaClient {
    /// Check if a JSON object contains known Bugzilla data keys,
    /// indicating the response has real data alongside any error fields.
    fn has_data_fields(map: &serde_json::Map<String, serde_json::Value>) -> bool {
        DATA_KEYS.iter().any(|key| map.contains_key(*key))
    }

    pub fn new(config: BugzillaClientConfig<'_>) -> Result<Self> {
        let BugzillaClientConfig {
            base_url,
            credential,
            auth_method,
            api_mode,
            email_hint,
            tls_config,
        } = config;

        let auth = match auth_method {
            AuthMethod::Header => {
                let value = HeaderValue::from_str(credential)
                    .map_err(|_| BzrError::config("invalid API key characters"))?;
                PreparedAuth::Header(value)
            }
            AuthMethod::QueryParam => PreparedAuth::QueryParam(credential.to_string()),
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
        let xmlrpc = Some(XmlRpcClient::new(http.clone(), base_url, credential));

        tracing::debug!(base_url, %auth_method, %api_mode, "created Bugzilla client");

        Ok(BugzillaClient {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth,
            api_key: credential.to_string(),
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
        let value = Self::parse_body_to_value(&body, &safe_url)?;
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
        Self::parse_body_to_value(&body, &safe_url)
    }

    fn parse_body_to_value(body: &str, safe_url: &str) -> Result<serde_json::Value> {
        tracing::trace!(
            url = safe_url,
            body = &body[..body.len().min(2048)],
            "response body"
        );

        let value: serde_json::Value = serde_json::from_str(body).map_err(|e| {
            tracing::debug!(
                url = safe_url,
                error = %e,
                body_preview = &body[..body.len().min(512)],
                "JSON deserialization failed"
            );
            BzrError::Deserialize(format!(
                "failed to parse response from {safe_url}: {e}\nbody preview ({} chars): {}",
                body.chars().count().min(BODY_PREVIEW_MAX_BYTES),
                format_body_preview(body),
            ))
        })?;

        Self::check_bugzilla_200_error(&value, safe_url)?;
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
                match extractor(value) {
                    Ok(v) => return Ok(v),
                    Err(e) if first_error.is_none() => first_error = Some(e),
                    Err(_) => {}
                }
            }
        }

        // Second pass: try remaining candidates as fallbacks.
        for (key, extractor) in candidates {
            if !present_keys.contains(*key) {
                match extractor(value) {
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
        // Re-serialize the parsed Value so the user sees what envelope shape
        // the server actually sent — the only diagnostic available once
        // typed deserialization has failed against every known shape.
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
pub(super) mod test_helpers;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
