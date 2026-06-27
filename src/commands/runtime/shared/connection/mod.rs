//! Connect to a Bugzilla server with auto-configuration, split by concern:
//! [`target`] resolves the connection target and owns `ConnectContext`,
//! [`tls_trust`] handles TLS trust (TOFU / pin rotation / probing), and
//! [`detect`] runs settings detection and persistence. This module owns only
//! the top-level [`connect_and_configure`] orchestration.

mod detect;
mod target;
mod tls_trust;

#[cfg(test)]
pub(super) mod test_helpers;

use crate::client::BugzillaClient;
use crate::commands::runtime::invocation::CommandContext;
use crate::error::{BzrError, Result};

use detect::{detect_with_tofu_fallback, DetectOrClient};
use target::{resolve_connect_target, ConnectContext, ConnectTarget};
use tls_trust::{pin_current_cert_for_session, probe_cached_connection};

/// Connect to a Bugzilla server with auto-configuration.
///
/// On first connection to a server, detects the auth method when credentials
/// exist and the API mode, then persists these settings to the config file.
/// The server's configured email (if any) is stored in the client for
/// Bugzilla 5.0 whoami fallback.
///
/// When a TLS certificate error occurs and no trust mechanism is configured,
/// offers an interactive TOFU (trust-on-first-use) prompt. When a pinned
/// certificate has rotated, offers a rotation prompt.
pub async fn connect_and_configure(command: &CommandContext) -> Result<BugzillaClient> {
    let ConnectTarget {
        ctx,
        mut tls_config,
        cached_auth,
        cached_mode,
        pin_current_cert,
    } = resolve_connect_target(command)?;

    if let Some(command_name) = command.credential_requirement() {
        require_credentials_for_connection(&ctx, command_name)?;
    }

    if pin_current_cert {
        pin_current_cert_for_session(&ctx, &mut tls_config).await?;
    }

    if tls_config.insecure {
        tracing::warn!(
            "TLS certificate verification disabled for server '{}'",
            ctx.server_name
        );
    }

    // Cached credentialed servers need auth + mode; cached anonymous servers
    // need only mode. Inline servers are always uncached (no config entry), so
    // they take the detect path and persist nothing.
    let (auth, resolved_mode) = match (cached_auth, cached_mode) {
        (Some(method), Some(mode)) => {
            // Even with full cache, surface TLS errors at connect-time so
            // TOFU and pin-rotation prompts can fire. Skipped only when
            // verification is explicitly disabled (`tls_insecure`); for
            // pinned servers and custom-CA servers we still probe so a
            // rotated cert / issuer change is caught here rather than at
            // the first real API call.
            if let Some(client) = probe_cached_connection(&ctx, &tls_config).await? {
                return Ok(client);
            }
            (Some(method), mode)
        }
        (None, Some(mode)) if ctx.api_key.is_none() => {
            if let Some(client) = probe_cached_connection(&ctx, &tls_config).await? {
                return Ok(client);
            }
            (None, mode)
        }
        (Some(method), None) => {
            tracing::debug!("auth_method cached but api_mode missing; re-detecting");
            match detect_with_tofu_fallback(&ctx, &tls_config).await? {
                DetectOrClient::Client(client) => return Ok(client),
                DetectOrClient::Settings(settings) => {
                    ctx.persist_settings(&settings, false)?;
                    (Some(method), settings.api_mode)
                }
            }
        }
        _ => match detect_with_tofu_fallback(&ctx, &tls_config).await? {
            DetectOrClient::Client(client) => return Ok(client),
            DetectOrClient::Settings(settings) => {
                ctx.persist_settings(&settings, true)?;
                (settings.auth_method, settings.api_mode)
            }
        },
    };

    let api_mode = command.api().unwrap_or(resolved_mode);
    let client = ctx.build_client(auth, api_mode, &tls_config)?;
    Ok(client)
}

fn require_credentials_for_connection(ctx: &ConnectContext, command_name: &str) -> Result<()> {
    if ctx.api_key.is_some() {
        return Ok(());
    }
    Err(BzrError::Config(format!(
        "{command_name} requires credentials; configure api_key, api_key_env, \
         api_key_keyring, or pass --server-api-key-env with --server-url"
    )))
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
