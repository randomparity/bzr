mod attachment;
pub(crate) mod auth;
pub(crate) use auth::{
    detect_server_settings, detect_server_settings_without_auth, DetectedServerSettings,
};
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

use crate::bugzilla_auth::{AUTH_HEADER_NAME, AUTH_QUERY_PARAM};
use crate::error::{BzrError, Result};
use crate::types::common::{ApiMode, AuthMethod};
use crate::types::user::BugzillaUser;
use crate::xmlrpc::protocol::XmlRpcClient;

/// Default fields for user queries (basic info).
pub(super) const USER_FIELDS_BASIC: &str = "id,name,real_name,email,groups";
/// Extended fields for detailed user queries.
pub(super) const USER_FIELDS_DETAILED: &str = "id,name,real_name,email,can_login,groups";

/// Field detail level for APIs that return users.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserDetailLevel {
    Basic,
    Detailed,
}

impl UserDetailLevel {
    const fn include_fields(self) -> &'static str {
        match self {
            Self::Basic => USER_FIELDS_BASIC,
            Self::Detailed => USER_FIELDS_DETAILED,
        }
    }
}

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
    auth: Option<PreparedAuth>,
    pub(super) api_key: Option<String>,
    pub(super) api_mode: ApiMode,
    pub(super) xmlrpc: XmlRpcClient,
    /// Email hint for Bugzilla 5.0 compatibility (whoami fallback via user lookup).
    email_hint: Option<String>,
    /// Transient-retry budget (429 / 5xx / timeout). 0 disables retries.
    retry_max: u32,
}

/// Configuration needed to construct a [`BugzillaClient`].
#[non_exhaustive]
#[derive(Clone, Copy)]
pub struct BugzillaClientConfig<'a> {
    pub base_url: &'a str,
    pub credential: Option<&'a str>,
    pub auth_method: Option<AuthMethod>,
    pub api_mode: ApiMode,
    pub email_hint: Option<&'a str>,
    pub tls_config: &'a crate::tls::TlsConfig,
    pub request_timeout: std::time::Duration,
    pub retry_max: u32,
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
            request_timeout,
            retry_max,
        } = config;

        let auth = match (credential, auth_method) {
            (Some(key), Some(AuthMethod::Header)) => {
                let value = HeaderValue::from_str(key)
                    .map_err(|_| BzrError::config("invalid API key characters"))?;
                Some(PreparedAuth::Header(value))
            }
            (Some(key), Some(AuthMethod::QueryParam)) => {
                Some(PreparedAuth::QueryParam(key.to_string()))
            }
            (None, None) => None,
            (Some(_), None) => {
                return Err(BzrError::config(
                    "internal: credential provided without detected auth method",
                ));
            }
            (None, Some(_)) => {
                return Err(BzrError::config(
                    "internal: auth method provided without credential",
                ));
            }
        };

        let http = crate::tls::build_tls_client(tls_config, request_timeout)?;

        // Always construct the XML-RPC client — even in REST mode, some
        // methods (e.g. Group.get on Bugzilla 5.3+) require XML-RPC fallback
        // because the REST endpoint is broken for them.
        if api_mode != ApiMode::Rest && auth_method == Some(AuthMethod::Header) {
            tracing::info!(
                "XML-RPC always sends API key in request body, \
                 overriding configured header auth for XML-RPC calls"
            );
        }
        let xmlrpc = XmlRpcClient::new(http.clone(), base_url, credential);

        tracing::debug!(base_url, ?auth_method, %api_mode, "created Bugzilla client");

        Ok(BugzillaClient {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth,
            api_key: credential.map(String::from),
            api_mode,
            xmlrpc,
            email_hint: email_hint.map(String::from),
            retry_max,
        })
    }

    /// Override the transient-retry budget. Used by tests to exercise the
    /// retry path without mutating the process-wide `--retry` global (which
    /// would race other tests).
    #[cfg(test)]
    pub(crate) fn set_retry_max(&mut self, n: u32) {
        self.retry_max = n;
    }

    pub(super) fn url(&self, path: &str) -> String {
        format!("{}/rest/{}", self.base_url, path.trim_start_matches('/'))
    }

    pub(super) fn xmlrpc_client(&self) -> &XmlRpcClient {
        &self.xmlrpc
    }

    /// Dispatch an operation across the detected API mode. In Hybrid mode the
    /// XML-RPC path is tried first and a transport failure falls back to REST
    /// (the shape shared by the per-resource read methods). `op` names the
    /// operation for the fallback log line.
    pub(super) async fn dispatch_xmlrpc_first<T>(
        &self,
        op: &str,
        rest: impl AsyncFnOnce() -> Result<T>,
        xmlrpc: impl AsyncFnOnce() -> Result<T>,
    ) -> Result<T> {
        match self.api_mode {
            ApiMode::Rest => rest().await,
            ApiMode::XmlRpc => xmlrpc().await,
            ApiMode::Hybrid => match xmlrpc().await {
                Ok(v) => Ok(v),
                Err(e) if e.is_transport_failure() => {
                    tracing::info!(op, error = %e, "XML-RPC {op} failed, retrying via REST");
                    rest().await
                }
                Err(e) => Err(e),
            },
        }
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
    ///
    /// Inspects the response body for a Bugzilla HTTP-200 error envelope
    /// (`{"error":true,...}`) — some deployments report a rejected mutation
    /// with a 200 status, which a status-only check would treat as success.
    pub(super) async fn put_json(&self, path: &str, body: &impl serde::Serialize) -> Result<()> {
        let req = self.apply_auth(self.http.put(self.url(path)).json(body));
        let resp = self.send(req).await?;
        self.check_mutation_response(resp).await
    }

    /// Validate a mutation (PUT) response body for a 200-status error
    /// envelope. An empty body is treated as success; a non-empty body that
    /// isn't valid JSON surfaces as a deserialization error, matching the
    /// read path's strictness.
    async fn check_mutation_response(&self, resp: reqwest::Response) -> Result<()> {
        let safe_url = Self::safe_url(resp.url());
        let body = resp.text().await?;
        if body.trim().is_empty() {
            return Ok(());
        }
        Self::parse_body_to_value(&body, &safe_url)?;
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

    /// Apply auth credentials to a request. Infallible because any configured
    /// API key was validated at client construction time. Anonymous clients
    /// leave the request unchanged.
    pub(super) fn apply_auth(&self, builder: RequestBuilder) -> RequestBuilder {
        match &self.auth {
            Some(PreparedAuth::Header(value)) => {
                crate::bugzilla_auth::apply_auth_to_request(builder, Some(value), None)
            }
            Some(PreparedAuth::QueryParam(key)) => {
                crate::bugzilla_auth::apply_auth_to_request(builder, None, Some(key))
            }
            None => builder,
        }
    }

    /// Send a request, applying transient-failure retries (429 / 5xx / connect
    /// timeout) with exponential backoff when `--retry` is enabled. Each attempt
    /// also performs the 401 alternate-auth fallback (see [`Self::send_raw`]).
    /// The status of the final attempt is checked normally, so exhausted retries
    /// surface the usual `HttpStatus`/`Http` error (exit code 5).
    pub(super) async fn send(&self, builder: RequestBuilder) -> Result<reqwest::Response> {
        // A write must not be replayed after a 5xx or read timeout: the server
        // may have already applied it, so a retry would duplicate the effect.
        // We gate on `is_safe()` (GET/HEAD), not `is_idempotent()`, because
        // bzr's PUT `Bug.update` is not effect-idempotent — `--work-time` is
        // additive and `--comment` posts atomically, so a replayed PUT could
        // double-count work or duplicate a comment. 429s and connect failures
        // are provably un-processed and stay retryable for any method; an
        // undeterminable method is treated as unsafe.
        let safe = builder
            .try_clone()
            .and_then(|b| b.build().ok())
            .is_some_and(|r| r.method().is_safe());
        let mut attempt: u32 = 0;
        loop {
            // Clone for a possible retry; a non-cloneable body (streaming) can't
            // be replayed, so send it once without retry.
            let Some(this) = builder.try_clone() else {
                return self
                    .check_response_status(self.send_raw(builder).await?)
                    .await;
            };
            match self.send_raw(this).await {
                Ok(resp)
                    if attempt < self.retry_max
                        && crate::http::should_retry_status(resp.status().as_u16(), safe) =>
                {
                    let status = resp.status().as_u16();
                    let retry_after = resp
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(crate::http::parse_retry_after);
                    Self::sleep_before_retry(attempt, retry_after, &format!("HTTP {status}")).await;
                    attempt += 1;
                }
                Ok(resp) => return self.check_response_status(resp).await,
                Err(e) if attempt < self.retry_max && Self::is_transient(&e, safe) => {
                    Self::sleep_before_retry(attempt, None, &e.to_string()).await;
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Whether an error is a transient transport failure worth retrying for a
    /// request with the given safety (GET/HEAD).
    fn is_transient(err: &BzrError, safe: bool) -> bool {
        matches!(err, BzrError::Http(e) if crate::http::should_retry_transport(e, safe))
    }

    /// Sleep for the backoff interval before the next retry, logging the reason.
    async fn sleep_before_retry(
        attempt: u32,
        retry_after: Option<std::time::Duration>,
        reason: &str,
    ) {
        let delay = crate::http::backoff_delay(attempt, retry_after);
        tracing::warn!(
            attempt = attempt + 1,
            delay_ms = u64::try_from(delay.as_millis()).unwrap_or(u64::MAX),
            reason,
            "transient failure; retrying after backoff"
        );
        tokio::time::sleep(delay).await;
    }

    /// One request attempt: send, log, and perform the 401 alternate-auth
    /// fallback. Returns the raw response without status-checking so the caller
    /// ([`Self::send`]) can decide whether a retryable status warrants a retry.
    async fn send_raw(&self, builder: RequestBuilder) -> Result<reqwest::Response> {
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
        Ok(resp)
    }

    /// On 401, retry the request with the alternate auth method (header ↔ query param).
    /// Returns `Ok(Some(response))` if the retry succeeded, `Ok(None)` if the retry
    /// also failed or wasn't possible, or `Err` on transport-level failures.
    async fn retry_with_alternate_auth(
        &self,
        retry_builder: Option<RequestBuilder>,
    ) -> Result<Option<reqwest::Response>> {
        if self.auth.is_none() {
            return Ok(None);
        }
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
        match (&self.auth, self.api_key.as_deref()) {
            (Some(PreparedAuth::Header(_)), Some(api_key)) => {
                Ok(builder.query(&[(AUTH_QUERY_PARAM, api_key)]))
            }
            (Some(PreparedAuth::QueryParam(_)), Some(api_key)) => {
                let value = HeaderValue::from_str(api_key).map_err(|e| {
                    BzrError::Config(format!("API key contains invalid header characters: {e}"))
                })?;
                Ok(builder.header(AUTH_HEADER_NAME, value))
            }
            _ => Ok(builder),
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
            body = crate::http::utf8_prefix(body, BODY_TRACE_MAX_BYTES),
            "response body"
        );

        let value: serde_json::Value = serde_json::from_str(body).map_err(|e| {
            tracing::debug!(
                url = safe_url,
                error = %e,
                body_preview = crate::http::utf8_prefix(body, BODY_PREVIEW_MAX_BYTES),
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
            let body = match response.text().await {
                Ok(body) => body,
                // The body is unreadable, but the HTTP status is still
                // meaningful. Surface the read failure as the body text so the
                // error is reported with a real diagnostic rather than being
                // silently swallowed into an empty string.
                Err(e) => {
                    return Err(BzrError::HttpStatus {
                        status: status.as_u16(),
                        body: format!("<failed to read response body: {e}>"),
                    });
                }
            };
            tracing::debug!(
                %status,
                body = crate::http::utf8_prefix(&body, BODY_PREVIEW_MAX_BYTES),
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
const BODY_PREVIEW_MAX_BYTES: usize = crate::http::DIAGNOSTIC_BODY_PREVIEW_MAX_BYTES;

/// Maximum length of the response body logged at `trace` level. Larger than
/// [`BODY_PREVIEW_MAX_BYTES`] because trace logs are opt-in diagnostics where
/// seeing more of the payload outweighs message compactness.
const BODY_TRACE_MAX_BYTES: usize = 2048;

/// Format a response body for inclusion in a `BzrError::Deserialize` message.
///
/// Truncates to [`BODY_PREVIEW_MAX_BYTES`] on a UTF-8 char boundary,
/// appends `…` when truncated, runs the result through
/// [`crate::bugzilla_auth::redact_api_key`] to strip echoed-back API keys, and
/// collapses internal newlines and tabs to single spaces so the preview
/// stays on one line beneath the main error.
///
/// Called by `parse_json` when deserializing JSON fails.
fn format_body_preview(body: &str) -> String {
    let prefix = crate::http::utf8_prefix(body, BODY_PREVIEW_MAX_BYTES);
    let mut preview = String::with_capacity(prefix.len() + 4);
    preview.push_str(prefix);
    if prefix.len() < body.len() {
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

    crate::bugzilla_auth::redact_api_key(&collapsed)
}

#[cfg(test)]
pub(super) mod test_helpers;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
