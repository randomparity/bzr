//! Response-body handling for [`BugzillaClient`]: JSON parsing, Bugzilla
//! HTTP-200 error classification, multi-envelope tolerance, and redacted
//! body previews for diagnostics.

use serde::Deserialize;

use crate::error::{BzrError, Result};

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

fn default_error_code() -> i64 {
    -1
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
#[path = "response_tests.rs"]
mod tests;
