//! Response-body handling for [`BugzillaClient`]: JSON parsing, Bugzilla
//! HTTP-200 error classification, multi-envelope tolerance, and redacted
//! body previews for diagnostics.

use serde::{Deserialize, Deserializer};

use crate::error::{BzrError, Result};
use crate::types::bug::deserialize_optional_string_list;
use crate::types::{BugAdjacencyBug, BugAdjacencyError};

use super::BugzillaClient;

#[derive(Deserialize)]
struct ErrorResponse {
    #[serde(default)]
    error: bool,
    #[serde(default = "default_error_code", deserialize_with = "deserialize_code")]
    code: i64,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictAdjacencyEnvelope {
    #[serde(default)]
    bugs: Vec<StrictAdjacencyBug>,
    #[serde(default)]
    faults: Vec<StrictAdjacencyFault>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictAdjacencyBug {
    id: u64,
    #[serde(default, deserialize_with = "deserialize_optional_detail")]
    summary: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_detail")]
    status: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_detail")]
    resolution: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_detail")]
    product: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_string_list")]
    version: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_detail")]
    assigned_to: Option<String>,
    #[serde(
        rename = "assigned_to_detail",
        default,
        deserialize_with = "deserialize_present_object"
    )]
    _assigned_to_detail: Option<serde_json::Map<String, serde_json::Value>>,
    #[serde(default, deserialize_with = "deserialize_optional_detail")]
    last_change_time: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_detail")]
    target_milestone: Option<String>,
    blocks: Vec<u64>,
    depends_on: Vec<u64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictAdjacencyFault {
    id: serde_json::Value,
    #[serde(rename = "faultCode")]
    code: i64,
    #[serde(
        rename = "faultString",
        default,
        deserialize_with = "deserialize_present_string"
    )]
    _message: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictAdjacencyResourceError {
    error: bool,
    #[serde(deserialize_with = "deserialize_strict_resource_code")]
    code: i64,
    #[serde(
        rename = "message",
        default,
        deserialize_with = "deserialize_present_string"
    )]
    _message: Option<String>,
    #[serde(
        rename = "documentation",
        default,
        deserialize_with = "deserialize_present_string"
    )]
    _documentation: Option<String>,
}

fn deserialize_optional_detail<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
        .map(|value| value.filter(|detail| !detail.is_empty()))
}

fn deserialize_present_object<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<serde_json::Map<String, serde_json::Value>>, D::Error>
where
    D: Deserializer<'de>,
{
    serde_json::Map::<String, serde_json::Value>::deserialize(deserializer).map(Some)
}

fn deserialize_present_string<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer).map(Some)
}

fn deserialize_strict_resource_code<'de, D>(deserializer: D) -> std::result::Result<i64, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de;

    struct StrictResourceCodeVisitor;

    impl de::Visitor<'_> for StrictResourceCodeVisitor {
        type Value = i64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("the integer or exact decimal string 100, 101, or 102")
        }

        fn visit_i64<E: de::Error>(self, value: i64) -> std::result::Result<i64, E> {
            matches!(value, 100..=102)
                .then_some(value)
                .ok_or_else(|| E::custom("unsupported strict Bug.get resource code"))
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> std::result::Result<i64, E> {
            let value = i64::try_from(value).map_err(E::custom)?;
            self.visit_i64(value)
        }

        fn visit_str<E: de::Error>(self, value: &str) -> std::result::Result<i64, E> {
            match value {
                "100" => Ok(100),
                "101" => Ok(101),
                "102" => Ok(102),
                _ => Err(E::custom("unsupported strict Bug.get resource code")),
            }
        }
    }

    deserializer.deserialize_any(StrictResourceCodeVisitor)
}

fn default_error_code() -> i64 {
    -1
}

/// Bugzilla returns error codes as integers on some versions and as
/// strings on others (e.g. `"32610"` on Bugzilla 5.3). Accept both.
/// This remains specialized because signed API codes and the `-1` default
/// cannot use the shared unsigned number-or-string adapter.
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

/// Bugzilla response keys that can carry real data alongside an error
/// payload. When one of them holds actual content, the error is a
/// non-fatal server-side warning (e.g. from a Bugzilla extension) and the
/// data should be used. Presence alone is not enough — see
/// [`value_carries_data`].
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
    pub(super) async fn parse_strict_bug_adjacency_response(
        &self,
        response: reqwest::Response,
        requested: &str,
    ) -> Result<std::result::Result<BugAdjacencyBug, BugAdjacencyError>> {
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            if status.is_client_error() {
                return parse_strict_adjacency_resource_error(&body, requested);
            }
            return Err(BzrError::HttpStatus {
                status: status.as_u16(),
                body: crate::http::diagnostic_body_preview(&body),
            });
        }

        let envelope: StrictAdjacencyEnvelope = serde_json::from_str(&body).map_err(|error| {
            BzrError::DataIntegrity(format!(
                "invalid strict Bug.get response for '{requested}': {error}"
            ))
        })?;
        match (envelope.bugs.as_slice(), envelope.faults.as_slice()) {
            ([bug], []) => Ok(Ok(strict_bug_to_public(bug, requested)?)),
            ([], [fault]) => Ok(Err(strict_fault_to_public(fault, requested)?)),
            _ => Err(BzrError::DataIntegrity(format!(
                "strict Bug.get for '{requested}' must return exactly one bug or fault"
            ))),
        }
    }

    /// Check whether a JSON object carries real Bugzilla data alongside any
    /// error fields — i.e. a known data key whose value holds content.
    fn has_data_fields(map: &serde_json::Map<String, serde_json::Value>) -> bool {
        DATA_KEYS
            .iter()
            .any(|key| map.get(*key).is_some_and(value_carries_data))
    }

    /// Validate a mutation (PUT) response body for a 200-status error
    /// envelope. An empty body is treated as success; a non-empty body that
    /// isn't valid JSON surfaces as a deserialization error, matching the
    /// read path's strictness.
    pub(super) async fn check_mutation_response(&self, resp: reqwest::Response) -> Result<()> {
        let safe_url = Self::safe_url(resp.url());
        let body = resp.text().await?;
        if body.trim().is_empty() {
            return Ok(());
        }
        Self::parse_body_to_value(&body, &safe_url)?;
        Ok(())
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
            body = crate::bugzilla_auth::redact_api_key(crate::http::utf8_prefix(
                body,
                BODY_TRACE_MAX_BYTES,
            )),
            "response body"
        );

        let value: serde_json::Value = serde_json::from_str(body).map_err(|e| {
            tracing::debug!(
                url = safe_url,
                error = %e,
                body_preview = crate::bugzilla_auth::redact_api_key(crate::http::utf8_prefix(
                    body,
                    BODY_PREVIEW_MAX_BYTES,
                )),
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

    /// Detect Bugzilla error payloads that arrive with HTTP 200 status.
    ///
    /// Some servers (e.g. IBM LTC Bugzilla) include error fields alongside
    /// valid data — only treat the error as fatal when the response doesn't
    /// also carry real data (a common Bugzilla result key holding content).
    ///
    /// The data must be non-empty, not merely present: a server answering a
    /// restricted-bug lookup with an error and an empty `bugs: []` placeholder
    /// has told us only the error, and swallowing it leaves an empty result
    /// that the caller reports as "bug not found" (issue #504, ADR 0015).
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

        let error = parse_error_response_value(value, url)?;
        let code = error.code;
        let message = error.message;
        let has_data = Self::has_data_fields(map);

        tracing::debug!(
            url,
            code,
            message = crate::bugzilla_auth::redact_api_key(message.as_deref().unwrap_or("unknown")),
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

    pub(super) async fn check_response_status(
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
                    let body = format!("<failed to read response body: {e}>");
                    return Err(BzrError::HttpStatus {
                        status: status.as_u16(),
                        body: crate::http::diagnostic_body_preview(&body),
                    });
                }
            };
            return Err(Self::error_from_status_body(status, &body));
        }
        Ok(response)
    }

    /// Classify an HTTP error status and its already-read body into the error
    /// bzr reports for it. Shared with the 401 alternate-auth fallback, which
    /// must consume the retried body to classify it and so cannot hand the
    /// response back to [`Self::check_response_status`] (ADR 0057).
    pub(super) fn error_from_status_body(status: reqwest::StatusCode, body: &str) -> BzrError {
        tracing::debug!(
            %status,
            body = crate::bugzilla_auth::redact_api_key(crate::http::utf8_prefix(
                body,
                BODY_PREVIEW_MAX_BYTES,
            )),
            "API error response"
        );
        if let Ok(err) = serde_json::from_str::<ErrorResponse>(body) {
            if err.error {
                return BzrError::Api {
                    code: err.code,
                    message: err.message.unwrap_or_else(|| status.to_string()),
                };
            }
        }
        BzrError::HttpStatus {
            status: status.as_u16(),
            body: crate::http::diagnostic_body_preview(body),
        }
    }

    /// The Bugzilla error code an error body carries, or `None` when the body
    /// is not a Bugzilla error envelope (`error: true`) or carries no `code`.
    /// [`ErrorResponse`] defaults a missing `code` to [`default_error_code`],
    /// and Bugzilla emits no code of `-1`, so reading that sentinel back as
    /// `None` cannot swallow a real code. `None` means the response offered no
    /// signal about *why* it was refused.
    pub(super) fn bugzilla_error_code(body: &str) -> Option<i64> {
        let envelope: ErrorResponse = serde_json::from_str(body).ok()?;
        (envelope.error && envelope.code != default_error_code()).then_some(envelope.code)
    }
}

fn parse_strict_adjacency_resource_error(
    body: &str,
    requested: &str,
) -> Result<std::result::Result<BugAdjacencyBug, BugAdjacencyError>> {
    let resource: StrictAdjacencyResourceError = serde_json::from_str(body).map_err(|error| {
        BzrError::DataIntegrity(format!(
            "invalid strict Bug.get resource error for '{requested}': {error}"
        ))
    })?;
    if !resource.error {
        return Err(BzrError::DataIntegrity(format!(
            "strict Bug.get resource response for '{requested}' is not an error"
        )));
    }
    Ok(Err(strict_adjacency_error(resource.code, requested)?))
}

fn strict_bug_to_public(wire: &StrictAdjacencyBug, requested: &str) -> Result<BugAdjacencyBug> {
    validate_strict_identity(wire.id, requested, "bug")?;
    validate_strict_id(wire.id, "bug ID")?;
    let mut blocks = wire.blocks.clone();
    let mut depends_on = wire.depends_on.clone();
    validate_strict_ids(&blocks, "blocks")?;
    validate_strict_ids(&depends_on, "depends_on")?;
    blocks.sort_unstable();
    blocks.dedup();
    depends_on.sort_unstable();
    depends_on.dedup();
    Ok(BugAdjacencyBug {
        id: wire.id,
        summary: wire.summary.clone(),
        status: wire.status.clone(),
        resolution: wire.resolution.clone(),
        product: wire.product.clone(),
        version: wire.version.clone(),
        assigned_to: wire.assigned_to.clone(),
        last_change_time: wire.last_change_time.clone(),
        target_milestone: wire.target_milestone.clone(),
        blocks,
        depends_on,
    })
}

fn strict_fault_to_public(
    fault: &StrictAdjacencyFault,
    requested: &str,
) -> Result<BugAdjacencyError> {
    validate_strict_fault_identity(&fault.id, requested)?;
    strict_adjacency_error(fault.code, requested)
}

fn strict_adjacency_error(code: i64, requested: &str) -> Result<BugAdjacencyError> {
    let numeric = super::parse_adjacency_numeric(requested).is_some();
    match code {
        100 if !numeric => Ok(BugAdjacencyError::NotFoundAlias),
        101 if numeric => Ok(BugAdjacencyError::NotFoundId),
        102 => Ok(BugAdjacencyError::Inaccessible),
        _ => Err(BzrError::DataIntegrity(format!(
            "strict Bug.get returned uncorrelated fault code {code} for '{requested}'"
        ))),
    }
}

fn validate_strict_fault_identity(identity: &serde_json::Value, requested: &str) -> Result<()> {
    if let Some(requested_id) = super::parse_adjacency_numeric(requested) {
        let observed = identity.as_i64().or_else(|| {
            identity
                .as_str()
                .and_then(|value| value.parse::<i64>().ok())
        });
        if observed == Some(requested_id) {
            return Ok(());
        }
    } else if identity.as_str() == Some(requested) {
        return Ok(());
    }
    Err(BzrError::DataIntegrity(format!(
        "strict Bug.get fault identity does not match requested '{requested}'"
    )))
}

fn validate_strict_identity(id: u64, requested: &str, kind: &str) -> Result<()> {
    if let Some(requested_id) = super::parse_adjacency_numeric(requested) {
        if id != u64::try_from(requested_id).unwrap_or(u64::MAX) {
            return Err(BzrError::DataIntegrity(format!(
                "strict Bug.get {kind} ID {id} does not match requested '{requested}'"
            )));
        }
    }
    Ok(())
}

fn validate_strict_ids(ids: &[u64], field: &str) -> Result<()> {
    for id in ids {
        validate_strict_id(*id, field)?;
    }
    Ok(())
}

fn validate_strict_id(id: u64, field: &str) -> Result<()> {
    if id > u64::try_from(i64::MAX).unwrap_or(u64::MAX) {
        return Err(BzrError::DataIntegrity(format!(
            "strict Bug.get {field} exceeds the signed 64-bit range"
        )));
    }
    Ok(())
}

/// Whether a Bugzilla data key's value actually carries a result.
///
/// An empty array, an empty object, and `null` are placeholders the server
/// emits beside an error, not data. Any other scalar counts as content.
fn value_carries_data(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::Object(map) => !map.is_empty(),
        _ => true,
    }
}

fn parse_error_response_value(value: &serde_json::Value, url: &str) -> Result<ErrorResponse> {
    serde_json::from_value(value.clone()).map_err(|e| {
        BzrError::Deserialize(format!(
            "failed to deserialize Bugzilla error response from {url}: {e}"
        ))
    })
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
    let preview = crate::http::diagnostic_body_preview(body);

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
#[path = "response_tests.rs"]
mod tests;
