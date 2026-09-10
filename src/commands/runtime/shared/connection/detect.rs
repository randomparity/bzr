//! Server-settings detection and persistence glue: running the auth/version
//! probes (with or without credentials), persisting detected settings under
//! the config lock, and the TOFU/rotation-aware detection wrapper.

use std::path::Path;

use crate::client::BugzillaClient;
use crate::client::DetectedServerSettings;
use crate::config::{Config, AUTH_METHOD_SOURCE_DETECTED};
use crate::error::Result;
use crate::tls::TlsConfig;
use crate::types::transport::AuthMethod;

use super::target::ConnectContext;
use super::tls_trust::classify_and_handle_tls_failure;

/// Persist detected server settings to config under the lock.
/// Persists `auth_method` when `persist_auth` is true and detection produced
/// one. Only persists `api_mode`/`server_version` when version detection
/// succeeded, and only stamps the `auth_method` provenance when the method was
/// genuinely probed (`auth_method_probed`) — a transport fallback stays unstamped
/// and is retried on the next connect (ADR-0069). Emits one `warn` when a
/// re-detection overturns a previously persisted method.
///
/// If the server was concurrently removed from disk, this is a no-op (we do
/// not resurrect a deleted server with detected settings) — logged, not silent.
pub(super) fn persist_detected_settings(
    config_path_override: Option<&Path>,
    server_name: &str,
    settings: &DetectedServerSettings,
    persist_auth: bool,
) -> Result<()> {
    Config::update_locked_at(config_path_override, |config| {
        let Some(srv) = config.servers.get_mut(server_name) else {
            tracing::debug!(
                "server '{server_name}' no longer in config; skipping settings persist"
            );
            return Ok(());
        };
        if persist_auth {
            if let Some(auth_method) = settings.auth_method {
                warn_on_auth_method_change(server_name, srv.auth_method, auth_method);
                srv.auth_method = Some(auth_method);
                if settings.auth_method_probed {
                    // Stamp the provenance only when the probe genuinely determined
                    // the method; a transport fallback stays unstamped and is
                    // retried on the next connect (ADR-0069).
                    srv.auth_method_source = Some(AUTH_METHOD_SOURCE_DETECTED.to_owned());
                }
            }
        }
        if settings.server_version.is_some() {
            srv.api_mode = Some(settings.api_mode);
            srv.server_version.clone_from(&settings.server_version);
        }
        Ok(())
    })?;
    Ok(())
}

/// Announce a re-detection that overturned a persisted `auth_method`.
///
/// This is the only user-visible signal that an upgrade changed how a server is
/// authenticated: the stale value produced partial results with exit 0 and no
/// diagnostic (ADR-0059), so the correction must not be silent either. Silent
/// when nothing changed, or on the first detection for a server, so a normal
/// connect stays quiet.
///
/// The message describes how to pin the old method rather than spelling out a
/// runnable command: `config set-server` replaces an entry rather than merging
/// into it, so a command short enough to log would drop the server's credential
/// and TLS settings if pasted.
fn warn_on_auth_method_change(
    server_name: &str,
    previous: Option<AuthMethod>,
    detected: AuthMethod,
) {
    let Some(previous) = previous else { return };
    if previous == detected {
        return;
    }
    tracing::warn!(
        "server '{server_name}': re-detected auth method {detected} (was {previous}); \
         if {previous} was deliberate, re-run the full `bzr config set-server {server_name}` \
         line that created this server with `--auth-method {previous}` added — set-server \
         replaces the entry, so keep its existing credential, email and TLS flags"
    );
}

/// Detect server settings and build a client, persisting the detected
/// settings to config. Shared tail logic for TOFU and pin rotation flows.
pub(super) async fn detect_and_build_client(
    ctx: &ConnectContext,
    tls_config: &TlsConfig,
) -> Result<BugzillaClient> {
    let settings = detect_settings(ctx, tls_config).await?;
    ctx.persist_settings(&settings, true)?;
    let api_mode = ctx.api_override.unwrap_or(settings.api_mode);
    ctx.build_client(settings.auth_method, api_mode, tls_config)
}

pub(super) async fn detect_settings(
    ctx: &ConnectContext,
    tls_config: &TlsConfig,
) -> Result<DetectedServerSettings> {
    if let Some(api_key) = ctx.api_key.as_deref() {
        crate::client::detect_server_settings(
            &ctx.url,
            api_key,
            ctx.email_hint(),
            tls_config,
            ctx.request_timeout,
        )
        .await
    } else {
        crate::client::detect_server_settings_without_auth(
            &ctx.url,
            tls_config,
            ctx.request_timeout,
        )
        .await
    }
}

/// Run `detect_server_settings` and handle TLS errors with TOFU or
/// pin rotation flows as appropriate.
pub(super) async fn detect_with_tofu_fallback(
    ctx: &ConnectContext,
    tls_config: &TlsConfig,
) -> Result<DetectOrClient> {
    let err = match detect_settings(ctx, tls_config).await {
        Ok(settings) => return Ok(DetectOrClient::Settings(settings)),
        Err(e) => e,
    };
    match classify_and_handle_tls_failure(&err, ctx, tls_config).await? {
        Some(client) => Ok(DetectOrClient::Client(client)),
        None => Err(err),
    }
}

/// Either detected settings (continue normal flow) or a fully-built
/// client (TOFU/rotation handled everything).
pub(super) enum DetectOrClient {
    Settings(DetectedServerSettings),
    Client(BugzillaClient),
}

#[cfg(test)]
#[path = "detect_tests.rs"]
mod tests;
