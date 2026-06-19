use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;
use std::time::Duration;

/// Kept short (10s) to fail fast on unreachable servers.
pub(crate) const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Per-request ceiling (30s) — covers large attachment downloads. Overridable
/// per invocation via `--timeout` / `BZR_TIMEOUT` (see [`request_timeout`]).
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Process-wide request-timeout override, set once from `--timeout` /
/// `BZR_TIMEOUT` before any client is built. `None` keeps [`REQUEST_TIMEOUT`].
static REQUEST_TIMEOUT_OVERRIDE: RwLock<Option<Duration>> = RwLock::new(None);
/// Process-wide retry budget for transient (429 / 5xx / timeout) failures, set
/// from `--retry`. 0 (the default) disables retries.
static RETRY_MAX: AtomicU32 = AtomicU32::new(0);

/// Base unit for exponential backoff between transient retries.
const RETRY_BACKOFF_BASE: Duration = Duration::from_millis(500);
/// Upper bound on any single backoff sleep, including a server `Retry-After`.
const RETRY_BACKOFF_CAP: Duration = Duration::from_secs(30);

/// Install the request-timeout override (seconds). Called once at startup,
/// before any [`crate::client::BugzillaClient`] is constructed.
pub fn set_request_timeout_secs(secs: Option<u64>) {
    let dur = secs.map(Duration::from_secs);
    if let Ok(mut guard) = REQUEST_TIMEOUT_OVERRIDE.write() {
        *guard = dur;
    }
}

/// The effective per-request timeout: the override if set, else the default.
pub(crate) fn request_timeout() -> Duration {
    REQUEST_TIMEOUT_OVERRIDE
        .read()
        .ok()
        .and_then(|g| *g)
        .unwrap_or(REQUEST_TIMEOUT)
}

/// Install the transient-retry budget. Called once at startup.
pub fn set_retry_max(n: u32) {
    RETRY_MAX.store(n, Ordering::Relaxed);
}

/// The configured transient-retry budget (0 = retries disabled).
pub(crate) fn retry_max() -> u32 {
    RETRY_MAX.load(Ordering::Relaxed)
}

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

/// Bugzilla's non-standard auth header (not `Authorization`).
pub(crate) const AUTH_HEADER_NAME: &str = "X-BUGZILLA-API-KEY";
/// Bugzilla's query-param auth key — used by servers that reject header auth.
pub(crate) const AUTH_QUERY_PARAM: &str = "Bugzilla_api_key";

/// Apply a pre-validated header value or query-param key to a request builder.
///
/// This is the shared auth-application primitive. Both the pre-client
/// [`apply_auth`] and [`crate::client::BugzillaClient::apply_auth`] delegate here.
pub(crate) fn apply_auth_to_request(
    builder: reqwest::RequestBuilder,
    header: Option<&reqwest::header::HeaderValue>,
    query_key: Option<&str>,
) -> reqwest::RequestBuilder {
    if let Some(val) = header {
        builder.header(AUTH_HEADER_NAME, val.clone())
    } else if let Some(key) = query_key {
        builder.query(&[(AUTH_QUERY_PARAM, key)])
    } else {
        builder
    }
}

/// Apply auth credentials to a request builder based on the configured method.
///
/// This is the fallible version used during auth detection (before a
/// [`crate::client::BugzillaClient`] is constructed). Returns `Err` if the
/// API key contains characters invalid for HTTP headers.
pub(crate) fn apply_auth(
    builder: reqwest::RequestBuilder,
    api_key: &str,
    method: crate::types::AuthMethod,
) -> crate::error::Result<reqwest::RequestBuilder> {
    match method {
        crate::types::AuthMethod::Header => {
            let val = reqwest::header::HeaderValue::from_str(api_key).map_err(|_| {
                crate::error::BzrError::config("API key contains invalid header characters")
            })?;
            Ok(apply_auth_to_request(builder, Some(&val), None))
        }
        crate::types::AuthMethod::QueryParam => {
            Ok(apply_auth_to_request(builder, None, Some(api_key)))
        }
    }
}

/// Check if an error message string contains TLS-related keywords.
pub(crate) fn looks_like_tls_error(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("cert") || lower.contains("ssl") || lower.contains("tls")
}

/// Pure predicate underlying [`is_tls_cert_error`], split out so the
/// connect-and-TLS-keyword logic can be unit tested without a live
/// `reqwest::Error` (which has no public constructor). Also called
/// directly by `format_http_error` to avoid recomputing the error chain.
pub(crate) fn is_connect_tls_error(is_connect: bool, error_chain: &str) -> bool {
    is_connect && looks_like_tls_error(error_chain)
}

/// Check if a reqwest error looks like a TLS certificate verification failure.
pub(crate) fn is_tls_cert_error(err: &reqwest::Error) -> bool {
    is_connect_tls_error(err.is_connect(), &crate::error::format_error_chain(err))
}

/// Hint text appended to TLS certificate errors.
pub(crate) const TLS_HINT: &str =
    "\n  hint: to trust this server's certificate, re-run interactively,\n    \
     or pre-pin with:  bzr config set-server <NAME> --tls-pin-now\n    \
     or provide a CA:  bzr config set-server <NAME> --tls-ca-cert <PATH>\n    \
     or skip verification: bzr config set-server <NAME> --tls-insecure";

/// Append a `--tls-insecure` hint to a message when a TLS certificate
/// error is detected, returning the enriched string.
pub(crate) fn tls_hint(base_msg: &str, err: &reqwest::Error) -> String {
    let mut msg = base_msg.to_string();
    if is_tls_cert_error(err) {
        msg.push_str(TLS_HINT);
    }
    msg
}

/// Redact a Bugzilla API key value out of a string for safe display.
///
/// Looks for the literal `Bugzilla_api_key=` marker (the query-param
/// form Bugzilla uses) and replaces the value up to the next `&`,
/// `)`, or space with `[REDACTED]`. If the marker is absent the input
/// is returned unchanged.
pub(crate) fn redact_api_key(msg: &str) -> String {
    let marker = format!("{AUTH_QUERY_PARAM}=");
    if let Some(idx) = msg.find(&marker) {
        let prefix = &msg[..idx + marker.len()];
        let rest = &msg[idx + marker.len()..];
        let end = rest.find(['&', ')', ' ']).unwrap_or(rest.len());
        format!("{prefix}[REDACTED]{}", &rest[end..])
    } else {
        msg.to_string()
    }
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
