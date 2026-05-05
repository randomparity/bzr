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

use super::version::detect_version_and_mode;

/// Result of server settings detection -- auth method, API mode, and
/// optionally the server version string. Returned by [`detect_server_settings`]
/// for the caller to persist as appropriate.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct DetectedServerSettings {
    pub auth_method: AuthMethod,
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
) -> Result<DetectedServerSettings> {
    let http = crate::tls::build_tls_client(tls_config)?;

    let method = detect_auth_method(&http, url, api_key, email).await?;
    let (version, api_mode) = detect_version_and_mode(&http, url, api_key, method).await;

    tracing::info!(
        %method,
        %api_mode,
        version = version.as_deref().unwrap_or("unknown"),
        "detected server settings"
    );

    Ok(DetectedServerSettings {
        auth_method: method,
        api_mode,
        server_version: version,
    })
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

    // Try whoami endpoint first (Bugzilla 5.1+)
    let whoami = detect_whoami_auth(http, base, api_key, &key_header).await;
    match whoami {
        WhoamiOutcome::Authenticated(method) => return Ok(method),
        WhoamiOutcome::NotFound => {
            tracing::info!("falling back to rest/valid_login for older Bugzilla");
        }
        WhoamiOutcome::NetworkError => {
            tracing::warn!(
                "could not reach server during auth detection; \
                 defaulting to header auth"
            );
            return Ok(AuthMethod::Header);
        }
        WhoamiOutcome::AuthRejected | WhoamiOutcome::UnparseableResponse => {}
    }

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
            ValidLoginOutcome::NetworkError => {
                tracing::warn!(
                    "valid_login probes failed due to network error; \
                     defaulting to header auth"
                );
                return Ok(AuthMethod::Header);
            }
            ValidLoginOutcome::AuthRejected => {}
        }
    }

    let hint = match whoami {
        WhoamiOutcome::NotFound if email.is_none() => {
            "auth detection failed: rest/whoami not available and no \
             --email provided for rest/valid_login fallback. \
             Re-run `bzr config set-server` with --email your@email."
        }
        WhoamiOutcome::NotFound => {
            "auth detection failed: rest/valid_login did not confirm \
             your credentials. Check your API key and email address."
        }
        _ => {
            "auth detection failed: could not authenticate with the \
             server. Check your API key and server URL."
        }
    };
    Err(BzrError::Auth(hint.into()))
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
