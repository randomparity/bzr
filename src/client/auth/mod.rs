//! Auth detection orchestrator — split into submodules because the two
//! probing strategies (`whoami` for Bugzilla 5.3+/BMO-derived servers,
//! `valid_login` for Bugzilla 5.0/5.2)
//! are each self-contained with their own types and logic.
//!
//! # Auth detection decision matrix
//!
//! The intended, behavior-locking matrix for credentialed detection. Each row
//! is exercised by a test (see `mod_tests.rs`, `whoami_tests.rs`,
//! `valid_login_tests.rs`) or by the connection layer's tests
//! (`commands/runtime/shared/connection/`). Changing any cell is a behavior
//! change and should update both this table and the corresponding test.
//!
//! ## Endpoint order
//!
//! 1. `rest/whoami` (Bugzilla 5.3+/BMO-derived). On `404`, fall back to:
//! 2. `rest/valid_login` (Bugzilla 5.0/5.2) — only attempted when an email is
//!    configured, since `valid_login` requires a `login` parameter.
//!
//! ## Auth-method order (within each endpoint)
//!
//! Header auth (`X-BUGZILLA-API-KEY`) is probed before query-param auth, and is
//! preferred when both work — it keeps the API key out of URLs (and out of
//! server logs / `safe_url` redaction paths).
//!
//! ## Probe-outcome classification
//!
//! | Server response                       | Outcome              | Effect                                                        |
//! |---------------------------------------|----------------------|---------------------------------------------------------------|
//! | `2xx` + valid body (`id>0` / `true`)  | `Authenticated`      | Detection succeeds with that auth method.                     |
//! | `2xx` + unparseable/anomalous body    | `MalformedResponse`  | Try the other method; preserved for the error diagnostic.     |
//! | `whoami` returns `404`                | `NotFound`           | Fall back to `valid_login`.                                   |
//! | non-`2xx`, or `whoami` `id==0`, or `valid_login` `false` | `AuthRejected` | Try the other method; otherwise detection fails. |
//! | TLS-certificate transport failure     | `NetworkError`→`Err` | Propagated as [`BzrError::Http`] so TOFU / pin-rotation fires.|
//! | any other transport failure (DNS, timeout, reset) | `NetworkError`→`Ok` | Defaults to [`AuthMethod::Header`]; detection is not retried. |
//!
//! ## Malformed-diagnostic precedence
//!
//! When several probes return malformed `200`s, the **first** malformed response
//! is preserved (`get_or_insert`) and surfaced in the final [`BzrError::Auth`]:
//! header before query-param within an endpoint, and `whoami` before
//! `valid_login` across endpoints. This keeps the diagnostic stable regardless
//! of how many later probes also misbehave.
//!
//! ## `valid_login` query-param + header REST verification
//!
//! Some servers (e.g. IBM LTC) reject header auth via `valid_login` but accept
//! it on real API endpoints. When `valid_login` reports query-param, header auth
//! is re-verified by a differential probe over `rest/user?names=<login>`, sent
//! three ways -- with the header, with no credentials, and with the query
//! parameter `valid_login` proved. Bodies are compared as parsed JSON; a leg's
//! status gates it but is never folded into the compared value, and the
//! anonymous re-check compares status class separately.
//!
//! | Observation                                                       | Effect           |
//! |-------------------------------------------------------------------|------------------|
//! | any leg fails to send or its body is unreadable                    | keep query-param |
//! | any leg's status is neither `2xx` nor `401`/`403`                  | keep query-param |
//! | the header or query-param leg is non-`2xx`, or carries a 200 error | keep query-param |
//! | header body == anonymous body                                      | keep query-param |
//! | header body matches neither peer                                   | keep query-param |
//! | header body == query-param body, anonymous response did not repeat | keep query-param |
//! | header body == query-param body, anonymous response repeated       | prefer header    |
//!
//! A `401`/`403` on the *anonymous* leg is kept deliberately: an anonymous
//! caller being refused what a credentialed one receives is discrimination, not
//! an inconclusive leg, and Bugzilla delivers that refusal as a status and an
//! error body together. Because the anonymous leg is a single observation that
//! the whole differential rests on, it is re-issued once before header auth is
//! preferred, and must return the same status class and body -- a one-off
//! rate-limit or WAF response, refusal or `200` interstitial alike, would
//! otherwise confirm header auth on a server that ignores the header. A 2xx
//! alone is not evidence either: `rest/bug` answers 200 anonymously, so the
//! probe this replaced could not fail for the condition it verified (ADR 0056).
//! Every inconclusive outcome keeps the method `valid_login` proved, and each
//! terminal decline says so at `info`.
//!
//! ## Cached vs. fresh detection (connection layer)
//!
//! | Cached state (credentialed)        | Behavior                                                            |
//! |------------------------------------|--------------------------------------------------------------------|
//! | auth + mode both cached            | No re-detection; TLS-probe only (so cert rotation still surfaces).  |
//! | auth cached, mode missing          | Re-detect mode; persist with `persist_auth=false` (keep cached auth).|
//! | auth missing, mode cached          | Full detect; persist with `persist_auth=true` (treated as uncached for auth).|
//! | nothing cached                     | Full detect; persist with `persist_auth=true`.                      |
//!
//! In every persist path, `api_mode`/`server_version` are written only when the
//! version probe succeeded (`server_version.is_some()`); a transient version
//! failure must not overwrite a previously-good cached mode.

mod valid_login;
mod whoami;

use reqwest::header::HeaderValue;

use crate::error::{BzrError, Result};
use crate::types::transport::{ApiMode, AuthMethod};

pub(super) use self::valid_login::prove_valid_login_current_method;
use self::valid_login::{detect_valid_login_auth, verify_header_auth_via_rest, ValidLoginOutcome};
use self::whoami::{detect_whoami_auth, WhoamiOutcome};

use super::version::{detect_version_and_mode, detect_version_and_mode_without_auth_checked};

const AUTH_PROBE_BODY_TRACE_MAX_BYTES: usize = 2048;

/// Bound a probe response body for `trace` logging, with the API key removed.
///
/// The query-parameter probes put the key in the request URL, and an error page
/// from a proxy or `CGI::Carp` typically echoes the request URI back in its body —
/// so a traced body can carry the key even though bzr never wrote it there. Both
/// probe call sites trace the body before checking the status, so this covers
/// error pages as well as successful responses. Matches what `response.rs` does
/// for its own body previews.
fn trace_body_preview(body: &str) -> String {
    // Move the cut before any key it would split, *then* redact. Truncating
    // first defeats the bare-key branch of `redact_api_key`, which matches the
    // active key by equality and so cannot match a half of it — leaving a
    // credential prefix in the trace line.
    let prefix = crate::http::utf8_prefix(body, AUTH_PROBE_BODY_TRACE_MAX_BYTES);
    let end = crate::bugzilla_auth::safe_api_key_preview_boundary(body, prefix.len());
    crate::bugzilla_auth::redact_api_key(&body[..end])
}

#[derive(Debug, Clone)]
pub(super) struct MalformedProbeResponse {
    probe: &'static str,
    method: AuthMethod,
    error: String,
}

impl MalformedProbeResponse {
    fn new(probe: &'static str, method: AuthMethod, error: impl std::fmt::Display) -> Self {
        Self {
            probe,
            method,
            error: error.to_string(),
        }
    }
}

impl std::fmt::Display for MalformedProbeResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {} probe returned malformed 200 response: {}",
            self.probe, self.method, self.error
        )
    }
}

/// Result of server settings detection -- auth method, API mode, and
/// optionally the server version string. Returned by [`detect_server_settings`]
/// for the caller to persist as appropriate.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DetectedServerSettings {
    pub auth_method: Option<AuthMethod>,
    pub api_mode: ApiMode,
    /// `Some` when the version endpoint responded successfully; `None` on
    /// transient failures. Callers should only persist `api_mode` and
    /// `server_version` when this is `Some`.
    pub server_version: Option<String>,
}

/// Detect auth method, API mode, and server version via network probes.
///
/// This is a pure detection function -- it does not read or write any
/// configuration. The caller is responsible for caching and persisting
/// the returned [`DetectedServerSettings`].
pub async fn detect_server_settings(
    url: &str,
    api_key: &str,
    email: Option<&str>,
    tls_config: &crate::tls::TlsConfig,
    request_timeout: std::time::Duration,
) -> Result<DetectedServerSettings> {
    let http = crate::tls::build_tls_client(tls_config, request_timeout)?;

    let method = detect_auth_method(&http, url, api_key, email).await?;
    let (version, api_mode) = detect_version_and_mode(&http, url, api_key, method).await;

    tracing::info!(
        %method,
        %api_mode,
        version = version.as_deref().unwrap_or("unknown"),
        "detected server settings"
    );

    Ok(DetectedServerSettings {
        auth_method: Some(method),
        api_mode,
        server_version: version,
    })
}

/// Detect API mode and server version without credentials.
///
/// This is used for public read-only servers. No auth probes are sent, and the
/// returned settings intentionally have no auth method to cache.
pub async fn detect_server_settings_without_auth(
    url: &str,
    tls_config: &crate::tls::TlsConfig,
    request_timeout: std::time::Duration,
) -> Result<DetectedServerSettings> {
    let http = crate::tls::build_tls_client(tls_config, request_timeout)?;
    let (version, api_mode) = detect_version_and_mode_without_auth_checked(&http, url).await?;

    tracing::info!(
        %api_mode,
        version = version.as_deref().unwrap_or("unknown"),
        "detected anonymous server settings"
    );

    Ok(DetectedServerSettings {
        auth_method: None,
        api_mode,
        server_version: version,
    })
}

/// Render a probe transport error with the API key removed.
///
/// `reqwest::Error`'s `Display` appends ` for url (<url>)` whenever a URL is
/// attached to the error, and the query-parameter probes carry the API key in
/// that query string. Formatting the error verbatim would therefore write the
/// key to stderr on any timeout, reset, or DNS failure.
///
/// Two seams, composed, because neither alone is complete. `safe_url` reduces
/// the attached URL to origin and path, which is what keeps the message useful
/// for diagnosis — but it rewrites only the exact string reqwest attached, and
/// an error's source chain can render a differently-encoded copy. So the result
/// then goes through [`crate::bugzilla_auth::redact_api_key`], the same
/// marker- and thread-local-based redaction that guards the user-facing
/// `BzrError::Http` display seam, which catches the key wherever it appears.
///
/// This is the single seam for every transport error raised under
/// `src/client/auth/`; adding a probe means routing its error through here.
pub(super) fn redacted_probe_error(error: &reqwest::Error) -> String {
    let rendered = format!("{error:#}");
    let without_url = match error.url() {
        Some(url) => rendered.replace(url.as_str(), &super::BugzillaClient::safe_url(url)),
        None => rendered,
    };
    crate::bugzilla_auth::redact_api_key(&without_url)
}

/// Log a probe's `send()` error, surfacing TLS-certificate problems at `warn`
/// with a [`crate::tls::tls_hint`] and routing all other transport errors to
/// `debug`. Shared by the `whoami` and `valid_login` probes so their
/// network-error handling cannot drift apart (the cause of TD-002).
fn log_probe_send_error(probe: &str, method: AuthMethod, e: &reqwest::Error) {
    let rendered = redacted_probe_error(e);
    if crate::tls::is_tls_cert_error(e) {
        tracing::warn!(
            "{}",
            crate::tls::tls_hint(&format!("{probe} {method} request failed: {rendered}"), e)
        );
    } else {
        tracing::debug!("{probe} {method} request failed: {rendered}");
    }
}

/// Decide what a probe transport failure means for auth detection.
///
/// A TLS-certificate failure is propagated as an error so the connection layer
/// can classify it and offer the TOFU / pin-rotation prompts (otherwise a
/// self-signed server could never be trusted on first contact). Every other
/// transport error (timeout, connection reset, DNS) falls back to header auth —
/// the safest default — rather than aborting: auth detection is not retried, and
/// the real request (which has the transient-retry budget) may still succeed, so
/// a single detection-time blip must not fail the whole invocation.
fn network_error_outcome(e: reqwest::Error) -> Result<AuthMethod> {
    if crate::tls::is_tls_cert_error(&e) {
        return Err(BzrError::Http(e));
    }
    // Not `{e:#}`: the query-param probes carry the API key in the URL reqwest
    // attaches to a transport error, and this arm logs at `warn`.
    tracing::warn!(
        "could not reach server during auth detection ({}); defaulting to header auth",
        redacted_probe_error(&e)
    );
    Ok(AuthMethod::Header)
}

fn auth_detection_error(hint: &str, malformed: Option<MalformedProbeResponse>) -> BzrError {
    let mut message = hint.to_owned();
    if let Some(malformed) = malformed {
        message.push_str(" Last malformed auth probe: ");
        message.push_str(&malformed.to_string());
        message.push('.');
    }
    BzrError::Auth(message)
}

async fn detect_auth_method(
    http: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    email: Option<&str>,
) -> Result<AuthMethod> {
    let base = base_url.trim_end_matches('/');

    if !base.starts_with("https://") {
        tracing::warn!(
            url = base,
            "server URL is not HTTPS -- API key will be sent in plaintext"
        );
    }

    let key_header = HeaderValue::from_str(api_key)
        .map_err(|_| BzrError::config("invalid API key characters"))?;

    let mut malformed_response = None;

    // Try whoami first (Bugzilla 5.3+/BMO-derived). A TLS-certificate failure is
    // propagated so the connection layer can offer TOFU / pin-rotation; other
    // transport errors fall back to header auth (see network_error_outcome).
    let whoami = detect_whoami_auth(http, base, api_key, &key_header).await;
    let whoami_not_found = match whoami {
        WhoamiOutcome::Authenticated(method) => return Ok(method),
        WhoamiOutcome::NetworkError(e) => return network_error_outcome(e),
        WhoamiOutcome::NotFound => {
            tracing::info!("falling back to rest/valid_login for older Bugzilla");
            true
        }
        WhoamiOutcome::AuthRejected => false,
        WhoamiOutcome::MalformedResponse(error) => {
            malformed_response.get_or_insert(error);
            false
        }
    };

    // Fall back to valid_login on Bugzilla 5.0/5.2 (requires email).
    if let Some(login) = email {
        match detect_valid_login_auth(http, base, api_key, &key_header, login).await {
            ValidLoginOutcome::Authenticated(method) => {
                // valid_login can give false negatives for header auth on servers
                // with custom extensions (e.g. IBM LTC). When query_param is
                // detected, verify by probing a real endpoint with header auth.
                // Prefer header when both work -- it avoids leaking keys in URLs.
                if method == AuthMethod::QueryParam
                    && verify_header_auth_via_rest(http, base, api_key, &key_header, login).await
                {
                    tracing::info!(
                        "header auth works on API endpoints despite valid_login \
                         rejecting it; preferring header"
                    );
                    return Ok(AuthMethod::Header);
                }
                return Ok(method);
            }
            ValidLoginOutcome::NetworkError(e) => return network_error_outcome(e),
            ValidLoginOutcome::AuthRejected => {}
            ValidLoginOutcome::MalformedResponse(error) => {
                malformed_response.get_or_insert(error);
            }
        }
    }

    let hint = if whoami_not_found && email.is_none() {
        "auth detection failed: Bugzilla 5.3+/BMO-derived servers use native whoami; \
         this Bugzilla 5.0/5.2 server needs an email for the rest/valid_login fallback. \
         Configure a named server by rerunning its complete \
         `bzr config set-server <name> --url <url> --email <email> ...` command, \
         preserving its existing options; or add `--server-email` to an inline \
         `--server-url` invocation."
    } else if whoami_not_found {
        "auth detection failed: rest/valid_login did not confirm \
         your credentials. Check your API key and email address."
    } else {
        "auth detection failed: could not authenticate with the \
         server. Check your API key and server URL."
    };
    Err(auth_detection_error(hint, malformed_response))
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
