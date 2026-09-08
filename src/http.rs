use std::time::Duration;

/// Kept short (10s) to fail fast on unreachable servers.
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Per-request ceiling (30s) — covers large attachment downloads. Overridable
/// per invocation via `--timeout` / `BZR_TIMEOUT`.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Shared byte cap for short diagnostic response-body previews.
pub(crate) const DIAGNOSTIC_BODY_PREVIEW_MAX_BYTES: usize = 512;

/// Ceiling on any response body buffered into memory. A base64 `data` field
/// costs 4/3, so 64 MiB admits an attachment of roughly 48 MiB — well above
/// Bugzilla's documented 1000 KB `maxattachmentsize` default. ADR 0068 records
/// why this number rather than a smaller one, and why no knob raises it.
pub(crate) const MAX_RESPONSE_BODY_BYTES: u64 = 64 * 1024 * 1024;

/// Why a bounded body read did not produce a body.
///
/// Kept distinct from [`crate::error::BzrError`] so each call site can tell a
/// refusal from a transport failure and keep the disposition it already had
/// for the latter.
#[derive(Debug)]
pub(crate) enum BodyReadError {
    /// The server sent more than the limit; the read stopped there.
    TooLarge { operation: String, limit_bytes: u64 },
    /// The body could not be read at all.
    Transport(reqwest::Error),
}

impl From<BodyReadError> for crate::error::BzrError {
    fn from(error: BodyReadError) -> Self {
        match error {
            BodyReadError::TooLarge {
                operation,
                limit_bytes,
            } => crate::error::BzrError::ResponseTooLarge {
                operation,
                limit_bytes,
                status: None,
            },
            BodyReadError::Transport(error) => crate::error::BzrError::Http(error),
        }
    }
}

/// Read a response body into a `String` under [`MAX_RESPONSE_BODY_BYTES`].
///
/// `operation` names what was being done, for the refusal message. Replaces
/// `reqwest::Response::text()`, which has no ceiling.
pub(crate) async fn read_body_bounded(
    response: reqwest::Response,
    operation: &str,
) -> std::result::Result<String, BodyReadError> {
    read_body_within(response, operation, MAX_RESPONSE_BODY_BYTES).await
}

/// [`read_body_bounded`] with an explicit limit, so the bound is provable
/// without a 64 MiB fixture and the auth probes can be driven at a test limit.
pub(crate) async fn read_body_within(
    mut response: reqwest::Response,
    operation: &str,
    limit_bytes: u64,
) -> std::result::Result<String, BodyReadError> {
    let mut body: Vec<u8> = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(BodyReadError::Transport)? {
        // Sum in u64: `usize` addition could wrap on a 32-bit target.
        if body.len() as u64 + chunk.len() as u64 > limit_bytes {
            return Err(BodyReadError::TooLarge {
                operation: operation.to_owned(),
                limit_bytes,
            });
        }
        body.extend_from_slice(&chunk);
    }
    // `from_utf8` reuses the buffer when the body is valid UTF-8 — the ordinary
    // case, and the one `Response::text()` pays a full copy for. Invalid UTF-8
    // still costs what `text()` costs: the failed `String` keeps the buffer
    // alive while `from_utf8_lossy` builds a replacement that can reach three
    // bytes per input byte, so an accepted body of pathological bytes peaks
    // near seven times the limit. That is not a regression and is bounded by
    // the limit; it is why the ceiling is stated per-path rather than flat.
    // The Vec is never pre-sized from Content-Length: the server writes it.
    Ok(String::from_utf8(body)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned()))
}

/// Base unit for exponential backoff between transient retries.
const RETRY_BACKOFF_BASE: Duration = Duration::from_millis(500);
/// Upper bound on any single backoff sleep, including a server `Retry-After`.
const RETRY_BACKOFF_CAP: Duration = Duration::from_secs(30);

/// Resolve the request-timeout override from the `--timeout` flag and the
/// `BZR_TIMEOUT` environment value. The flag wins (already validated `>= 1` by
/// clap); otherwise the env value is accepted only when a positive integer. An
/// invalid env value yields `None` (the caller keeps the default and may warn).
pub fn resolve_timeout_secs(flag: Option<u64>, env: Option<&str>) -> Option<u64> {
    if let Some(secs) = flag {
        return Some(secs);
    }
    env.and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|&s| s >= 1)
}

/// Whether an HTTP status is transient in principle: 429 (rate limited) or any
/// 5xx (server error). Other 4xx are caller errors. This does not consider
/// idempotency — see [`should_retry_status`].
pub(crate) fn is_retryable_status(status: u16) -> bool {
    status == 429 || (500..600).contains(&status)
}

/// Whether a response status should actually be retried, given whether the
/// request is safe (no side effects — GET/HEAD). 429 means the request was
/// rate-limited before processing, so it is always retryable; a 5xx may have
/// been applied server-side before the error surfaced, so it is retried only
/// for safe requests — retrying a write (POST `Bug.create`, PUT `Bug.update`
/// with `--work-time`/`--comment`) could duplicate the effect.
pub(crate) fn should_retry_status(status: u16, safe: bool) -> bool {
    status == 429 || (safe && is_retryable_status(status))
}

/// Whether a transport-level error should be retried, given whether the request
/// is safe (GET/HEAD). A connect failure means the server never received the
/// request, so it is always retryable; a read timeout may have been processed
/// before the timeout fired, so it is retried only for safe requests.
pub(crate) fn should_retry_transport(err: &reqwest::Error, safe: bool) -> bool {
    err.is_connect() || (safe && err.is_timeout())
}

/// Parse a `Retry-After` header value. Only the delta-seconds form (a bare
/// integer) is supported; the HTTP-date form is rare for `Retry-After` and is
/// treated as unknown (the caller falls back to exponential backoff) rather
/// than pulling in a date-parsing dependency for it.
pub(crate) fn parse_retry_after(value: &str) -> Option<Duration> {
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

/// Return the longest prefix at or below `max_bytes` without splitting UTF-8.
pub(crate) fn utf8_prefix(body: &str, max_bytes: usize) -> &str {
    let mut end = body.len().min(max_bytes);
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    &body[..end]
}

/// Return a human-scaled response-body excerpt without splitting UTF-8 or an API key.
pub(crate) fn diagnostic_body_preview(body: &str) -> String {
    let prefix = utf8_prefix(body, DIAGNOSTIC_BODY_PREVIEW_MAX_BYTES);
    let boundary = crate::bugzilla_auth::safe_api_key_preview_boundary(body, prefix.len());
    let mut preview = String::with_capacity(boundary + 3);
    preview.push_str(&body[..boundary]);
    if boundary < body.len() {
        preview.push('…');
    }
    preview
}

/// Backoff before retry `attempt` (0-based): `RETRY_BACKOFF_BASE * 2^attempt`,
/// capped at [`RETRY_BACKOFF_CAP`]. A server `Retry-After` is honored when it is
/// longer than the exponential base (and is itself capped), so the client never
/// waits less than the server asked nor more than the cap.
pub(crate) fn backoff_delay(attempt: u32, retry_after: Option<Duration>) -> Duration {
    let factor = 1u32 << attempt.min(5);
    let base = RETRY_BACKOFF_BASE
        .saturating_mul(factor)
        .min(RETRY_BACKOFF_CAP);
    match retry_after {
        Some(ra) => ra.min(RETRY_BACKOFF_CAP).max(base),
        None => base,
    }
}
/// Cap applied to opportunistic XML-RPC retries triggered when a primary
/// REST request succeeded with no rows. The full `REQUEST_TIMEOUT` is too
/// generous here: REST already returned an answer, and a slow XML-RPC
/// fallback shouldn't make the user pay 30s for a retry that may not
/// improve the result. 8s is enough for a healthy `Bug.search` against a
/// large database while still failing fast on truly unresponsive servers.
pub(crate) const XMLRPC_FALLBACK_TIMEOUT: Duration = Duration::from_secs(8);

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
