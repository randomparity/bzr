//! TLS trust handling for connection setup: classifying TLS verification
//! failures and driving the interactive trust-on-first-use (TOFU) prompt,
//! pin-rotation prompt, issuer-change hard failure, and the cached-connection
//! probe that surfaces those at connect time.

use crate::client::BugzillaClient;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::tls::TlsConfig;

use super::detect::detect_and_build_client;
use super::target::ConnectContext;

/// Check if a TLS error should trigger the TOFU (trust-on-first-use) flow.
///
/// Returns `true` when the error is a TLS certificate verification failure
/// and no trust mechanism (insecure, CA cert, or pin) is already configured.
pub(super) fn should_offer_tofu(err: &BzrError, tls_config: &TlsConfig) -> bool {
    if !tls_uses_default_trust(tls_config) {
        return false;
    }
    matches!(err, BzrError::Http(e) if crate::tls::is_tls_cert_error(e))
}

/// Check whether the connection relies on the default OS trust store with
/// no user-configured anchor (insecure flag, custom CA, or pinned cert).
///
/// When this returns `true`, TLS errors at first contact are eligible for
/// the TOFU prompt; when `false`, the user has already expressed how they
/// want the server's certificate verified and we don't override that.
pub(super) fn tls_uses_default_trust(tls_config: &TlsConfig) -> bool {
    !tls_config.insecure && tls_config.ca_cert_path.is_none() && tls_config.pin_sha256.is_none()
}

/// Probe TLS reachability with a single HEAD against the server URL.
///
/// Used on the cached connection path to surface certificate-verification
/// errors at connect time instead of deferring them to the first real API
/// call. The probe uses the user's configured `TlsConfig` (default trust
/// store, custom CA, or pin) so any handshake failure mirrors what the
/// real request would see.
///
/// Redirects are not followed: the probe must validate only the
/// certificate presented by the configured URL itself, otherwise a 301
/// to a different host would lead the prompt to describe one endpoint
/// while pinning (or PIN_MISMATCH-rotating against) another.
///
/// HTTP-level responses (any status) are reported as `Ok(())` — the goal
/// is purely to surface transport errors. Network/transport failures are
/// returned as the original `BzrError::Http` so callers can classify them
/// (TLS cert error, pin mismatch, etc.).
pub(super) async fn probe_tls(
    url: &str,
    tls_config: &TlsConfig,
    request_timeout: std::time::Duration,
) -> Result<()> {
    let client = crate::tls::build_probe_client(tls_config, request_timeout)?;
    match client.head(url).send().await {
        Ok(_) => Ok(()),
        Err(e) => Err(BzrError::Http(e)),
    }
}

/// Handle the TOFU flow: probe the server certificate, prompt the user,
/// and if accepted, retry detection and build the client.
// Mutation testing: this function only fires after a terminal-stdin
// TOFU prompt accepts; unit tests never hit it. cargo-mutants v27's
// exclude_re does not reliably match `delete field` mutations on struct
// expressions, so the function-level attribute is required.
#[cfg_attr(test, mutants::skip)]
pub(super) async fn handle_tofu(ctx: &ConnectContext) -> Result<BugzillaClient> {
    let hostname = ctx.hostname();
    let (fingerprint, issuer, issuer_der) =
        crate::tls::tofu::probe_server_cert(&ctx.url, ctx.request_timeout).await?;

    let decision =
        crate::tls::tofu::prompt_tofu(&ctx.server_name, &hostname, &fingerprint, &issuer)?;

    let tls_config = match decision {
        Some(true) => {
            // "always" — persist pin to config under the lock. `issuer_der` is
            // Option<String>; clone the values the closure needs before it (the
            // bare `fingerprint`/`issuer_der` are still used to build TlsConfig below).
            // An inline server has no config entry to pin against, so "always"
            // degrades to a session-only trust (`persist_locked` no-ops; the
            // TlsConfig below still applies for the rest of this invocation).
            let fingerprint_c = fingerprint.clone();
            let issuer_c = issuer.clone();
            let issuer_der_c = issuer_der.clone();
            let server_name = ctx.server_name.clone();
            ctx.persist_locked(move |config| {
                if let Some(srv) = config.servers.get_mut(&server_name) {
                    srv.tls_pin_sha256 = Some(fingerprint_c);
                    srv.tls_pin_issuer = Some(issuer_c);
                    srv.tls_pin_issuer_der = issuer_der_c;
                }
                Ok(())
            })?;
            TlsConfig {
                pin_sha256: Some(fingerprint),
                pin_issuer_der: issuer_der,
                server_name: Some(ctx.server_name.clone()),
                ..Default::default()
            }
        }
        Some(false) => {
            // "y" — trust this specific cert for this session only (no config change)
            TlsConfig {
                pin_sha256: Some(fingerprint),
                pin_issuer_der: issuer_der,
                server_name: Some(ctx.server_name.clone()),
                ..Default::default()
            }
        }
        None => {
            return Err(BzrError::config(
                "TLS certificate not trusted. To connect, use one of:\n  \
                 bzr config set-server <NAME> --tls-insecure\n  \
                 bzr config set-server <NAME> --tls-pin-sha256 <PIN>",
            ));
        }
    };

    detect_and_build_client(ctx, &tls_config).await
}

/// Handle pin mismatch (certificate rotated but issuer unchanged):
/// use the fingerprint and issuer parsed from the `PIN_MISMATCH` error,
/// prompt the user, and if accepted, update the pin and retry.
// Mutation testing: same rationale as handle_tofu above.
#[cfg_attr(test, mutants::skip)]
pub(super) async fn handle_pin_rotation(
    ctx: &ConnectContext,
    old_pin: &str,
    new_fingerprint: &str,
    new_issuer: &str,
) -> Result<BugzillaClient> {
    let hostname = ctx.hostname();

    let accepted = crate::tls::tofu::prompt_rotation(
        &ctx.server_name,
        &hostname,
        old_pin,
        new_fingerprint,
        new_issuer,
    )?;

    if !accepted {
        return Err(BzrError::config(format!(
            "certificate rotation rejected for server \"{server_name}\". \
             To clear the pin: bzr config set-server {server_name} \
             --tls-pin-clear",
            server_name = ctx.server_name
        )));
    }

    // Update pin in config. Keep the existing pin_issuer_der: since
    // PIN_MISMATCH only fires when the issuer DER matched (otherwise
    // ISSUER_CHANGED would have fired), the DER bytes are still valid.
    // Read existing issuer DER for the returned TlsConfig (PIN_MISMATCH implies
    // the issuer DER still matches, so it stays valid).
    let existing_issuer_der = Config::load_at(ctx.config_path_override.as_deref())
        .ok()
        .and_then(|c| {
            c.servers
                .get(&ctx.server_name)
                .and_then(|s| s.tls_pin_issuer_der.clone())
        });

    let new_fp = new_fingerprint.to_owned();
    let new_iss = new_issuer.to_owned();
    let server_name = ctx.server_name.clone();
    ctx.persist_locked(move |config| {
        if let Some(srv) = config.servers.get_mut(&server_name) {
            srv.tls_pin_sha256 = Some(new_fp);
            srv.tls_pin_issuer = Some(new_iss);
        }
        Ok(())
    })?;

    let tls_config = TlsConfig {
        pin_sha256: Some(new_fingerprint.to_owned()),
        pin_issuer_der: existing_issuer_der,
        server_name: Some(ctx.server_name.clone()),
        ..Default::default()
    };

    detect_and_build_client(ctx, &tls_config).await
}

/// Classify a TLS-layer failure and dispatch to the appropriate prompt.
///
/// Returns:
/// - `Ok(Some(client))` — TOFU or rotation fired and produced a client.
/// - `Ok(None)` — the error is not a TLS verification failure; caller
///   should propagate the original error (or, on the probe path, ignore
///   it and let the actual command surface it with full context).
/// - `Err(_)` — issuer changed (treated as a hard failure with a clear
///   remediation hint), or a downstream prompt/probe error.
pub(super) async fn classify_and_handle_tls_failure(
    err: &BzrError,
    ctx: &ConnectContext,
    tls_config: &TlsConfig,
) -> Result<Option<BugzillaClient>> {
    if should_offer_tofu(err, tls_config) {
        let client = handle_tofu(ctx).await?;
        return Ok(Some(client));
    }
    if let Some(pin_failure) = crate::tls::pin_failure::classify(err) {
        match pin_failure {
            crate::tls::pin_failure::TlsPinFailure::PinMismatch {
                expected,
                actual,
                new_issuer,
            } => {
                let client = handle_pin_rotation(ctx, &expected, &actual, &new_issuer).await?;
                return Ok(Some(client));
            }
            crate::tls::pin_failure::TlsPinFailure::IssuerChanged {
                expected_issuer,
                actual_issuer,
            } => {
                return Err(BzrError::IssuerChanged {
                    server: ctx.server_name.clone(),
                    expected_issuer,
                    actual_issuer,
                });
            }
        }
    }
    Ok(None)
}

pub(super) async fn pin_current_cert_for_session(
    ctx: &ConnectContext,
    tls_config: &mut TlsConfig,
) -> Result<()> {
    let (fingerprint, _issuer, issuer_der) =
        crate::tls::tofu::probe_server_cert(&ctx.url, ctx.request_timeout).await?;
    tls_config.pin_sha256 = Some(fingerprint);
    tls_config.pin_issuer_der = issuer_der;
    tls_config.server_name = Some(ctx.server_name.clone());
    Ok(())
}

pub(super) async fn probe_cached_connection(
    ctx: &ConnectContext,
    tls_config: &TlsConfig,
) -> Result<Option<BugzillaClient>> {
    if tls_config.insecure {
        return Ok(None);
    }

    if let Err(e) = probe_tls(&ctx.url, tls_config, ctx.request_timeout).await {
        if let Some(client) = classify_and_handle_tls_failure(&e, ctx, tls_config).await? {
            return Ok(Some(client));
        }
        // Non-TLS transport errors don't block: the actual command will hit
        // the same condition and report it with full request context.
    }

    Ok(None)
}

#[cfg(test)]
#[path = "tls_trust_tests.rs"]
mod tests;
