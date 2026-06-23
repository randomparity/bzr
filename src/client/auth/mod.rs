//! Auth detection orchestrator — split into submodules because the two
//! probing strategies (`whoami` for Bugzilla 5.1+, `valid_login` for 5.0+)
//! are each self-contained with their own types and logic.

mod valid_login;
mod whoami;

use reqwest::header::HeaderValue;

use crate::error::{BzrError, Result};
use crate::types::{ApiMode, AuthMethod};

use self::valid_login::{detect_valid_login_auth, verify_header_auth_via_rest, ValidLoginOutcome};
use self::whoami::{detect_whoami_auth, WhoamiOutcome};

use super::version::{detect_version_and_mode, detect_version_and_mode_without_auth_checked};

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
        WhoamiOutcome::AuthRejected | WhoamiOutcome::UnparseableResponse => false,
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
    Err(BzrError::Auth(hint.into()))
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
