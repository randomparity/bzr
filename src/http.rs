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

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
