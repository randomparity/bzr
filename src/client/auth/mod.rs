//! Auth detection orchestrator — split into submodules because the two
//! probing strategies (`whoami` for Bugzilla 5.1+, `valid_login` for 5.0+)
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
//! 1. `rest/whoami` (Bugzilla 5.1+). On `404` (endpoint absent), fall back to:
//! 2. `rest/valid_login` (Bugzilla 5.0+) — only attempted when an email is
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
//! is re-verified against `rest/bug`; if that succeeds, header is preferred.
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

use self::valid_login::{detect_valid_login_auth, verify_header_auth_via_rest, ValidLoginOutcome};
use self::whoami::{detect_whoami_auth, WhoamiOutcome};

use super::version::{detect_version_and_mode, detect_version_and_mode_without_auth_checked};

const AUTH_PROBE_BODY_TRACE_MAX_BYTES: usize = 2048;

fn trace_body_preview(body: &str) -> &str {
    crate::http::utf8_prefix(body, AUTH_PROBE_BODY_TRACE_MAX_BYTES)
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

/// Log a probe's `send()` error, surfacing TLS-certificate problems at `warn`
/// with a [`crate::tls::tls_hint`] and routing all other transport errors to
/// `debug`. Shared by the `whoami` and `valid_login` probes so their
/// network-error handling cannot drift apart (the cause of TD-002).
fn log_probe_send_error(probe: &str, method: AuthMethod, e: &reqwest::Error) {
    if crate::tls::is_tls_cert_error(e) {
        tracing::warn!(
            "{}",
            crate::tls::tls_hint(&format!("{probe} {method} request failed: {e:#}"), e)
        );
    } else {
        tracing::debug!("{probe} {method} request failed: {e:#}");
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
    tracing::warn!(
        "could not reach server during auth detection ({e:#}); defaulting to header auth"
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

    // Try whoami endpoint first (Bugzilla 5.1+). A TLS-certificate failure is
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

    // Fall back to valid_login endpoint (Bugzilla 5.0+, requires email)
    if let Some(login) = email {
        match detect_valid_login_auth(http, base, api_key, &key_header, login).await {
            ValidLoginOutcome::Authenticated(method) => {
                // valid_login can give false negatives for header auth on servers
                // with custom extensions (e.g. IBM LTC). When query_param is
                // detected, verify by probing a real endpoint with header auth.
                // Prefer header when both work -- it avoids leaking keys in URLs.
                if method == AuthMethod::QueryParam
                    && verify_header_auth_via_rest(http, base, &key_header).await
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
        "auth detection failed: rest/whoami not available and no \
         --email provided for rest/valid_login fallback. \
         Re-run `bzr config set-server` with --email your@email."
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
