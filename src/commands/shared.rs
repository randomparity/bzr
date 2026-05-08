use crate::client::BugzillaClient;
use crate::client::DetectedServerSettings;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::tls::TlsConfig;
use crate::types::{ApiMode, AuthMethod};

/// Persist detected server settings to config.
/// Always persists `auth_method` when `persist_auth` is true.
/// Only persists `api_mode` and `server_version` when version detection
/// succeeded (`server_version` is `Some`).
fn persist_detected_settings(
    config: &mut Config,
    server_name: &str,
    settings: &DetectedServerSettings,
    persist_auth: bool,
) -> Result<()> {
    if let Some(srv_mut) = config.servers.get_mut(server_name) {
        if persist_auth {
            srv_mut.auth_method = Some(settings.auth_method);
        }
        if settings.server_version.is_some() {
            srv_mut.api_mode = Some(settings.api_mode);
            srv_mut.server_version.clone_from(&settings.server_version);
        }
        config.save()?;
    }
    Ok(())
}

/// Check if a TLS error should trigger the TOFU (trust-on-first-use) flow.
///
/// Returns `true` when the error is a TLS certificate verification failure
/// and no trust mechanism (insecure, CA cert, or pin) is already configured.
fn should_offer_tofu(err: &BzrError, tls_config: &TlsConfig) -> bool {
    if !tls_uses_default_trust(tls_config) {
        return false;
    }
    matches!(err, BzrError::Http(e) if crate::http::is_tls_cert_error(e))
}

/// Check whether the connection relies on the default OS trust store with
/// no user-configured anchor (insecure flag, custom CA, or pinned cert).
///
/// When this returns `true`, TLS errors at first contact are eligible for
/// the TOFU prompt; when `false`, the user has already expressed how they
/// want the server's certificate verified and we don't override that.
fn tls_uses_default_trust(tls_config: &TlsConfig) -> bool {
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
async fn probe_tls(url: &str, tls_config: &TlsConfig) -> Result<()> {
    let client = crate::tls::build_probe_client(tls_config)?;
    match client.head(url).send().await {
        Ok(_) => Ok(()),
        Err(e) => Err(BzrError::Http(e)),
    }
}

/// Extract the hostname from a URL string, falling back to the raw URL
/// if parsing fails.
fn extract_hostname(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(String::from))
        .unwrap_or_else(|| url.to_string())
}

struct ConnectContext {
    server_name: String,
    url: String,
    api_key: String,
    email: Option<String>,
    api_override: Option<ApiMode>,
}

impl ConnectContext {
    fn email_hint(&self) -> Option<&str> {
        self.email.as_deref()
    }

    fn hostname(&self) -> String {
        extract_hostname(&self.url)
    }

    fn build_client(
        &self,
        auth_method: AuthMethod,
        api_mode: ApiMode,
        tls_config: &TlsConfig,
    ) -> Result<BugzillaClient> {
        BugzillaClient::new(crate::client::BugzillaClientConfig {
            base_url: &self.url,
            credential: &self.api_key,
            auth_method,
            api_mode,
            email_hint: self.email_hint(),
            tls_config,
        })
    }
}

/// Handle the TOFU flow: probe the server certificate, prompt the user,
/// and if accepted, retry detection and build the client.
// Mutation testing: this function only fires after a terminal-stdin
// TOFU prompt accepts; unit tests never hit it. cargo-mutants v27's
// exclude_re does not reliably match `delete field` mutations on struct
// expressions, so the function-level attribute is required.
#[cfg_attr(test, mutants::skip)]
async fn handle_tofu(ctx: &ConnectContext, config: &mut Config) -> Result<BugzillaClient> {
    let hostname = ctx.hostname();
    let (fingerprint, issuer, issuer_der) = crate::tls::tofu::probe_server_cert(&ctx.url).await?;

    let decision =
        crate::tls::tofu::prompt_tofu(&ctx.server_name, &hostname, &fingerprint, &issuer)?;

    let tls_config = match decision {
        Some(true) => {
            // "always" — persist pin to config
            if let Some(srv) = config.servers.get_mut(&ctx.server_name) {
                srv.tls_pin_sha256 = Some(fingerprint.clone());
                srv.tls_pin_issuer = Some(issuer.clone());
                srv.tls_pin_issuer_der.clone_from(&issuer_der);
                config.save()?;
            }
            TlsConfig {
                pin_sha256: Some(fingerprint),
                pin_issuer: Some(issuer),
                pin_issuer_der: issuer_der,
                server_name: Some(ctx.server_name.clone()),
                ..Default::default()
            }
        }
        Some(false) => {
            // "y" — trust this specific cert for this session only (no config change)
            TlsConfig {
                pin_sha256: Some(fingerprint),
                pin_issuer: Some(issuer),
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

    detect_and_build_client(ctx, &tls_config, config).await
}

/// Handle pin mismatch (certificate rotated but issuer unchanged):
/// use the fingerprint and issuer parsed from the `PIN_MISMATCH` error,
/// prompt the user, and if accepted, update the pin and retry.
// Mutation testing: same rationale as handle_tofu above.
#[cfg_attr(test, mutants::skip)]
async fn handle_pin_rotation(
    ctx: &ConnectContext,
    old_pin: &str,
    new_fingerprint: &str,
    new_issuer: &str,
    config: &mut Config,
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
    let existing_issuer_der = config
        .servers
        .get(&ctx.server_name)
        .and_then(|s| s.tls_pin_issuer_der.clone());
    if let Some(srv) = config.servers.get_mut(&ctx.server_name) {
        srv.tls_pin_sha256 = Some(new_fingerprint.to_owned());
        srv.tls_pin_issuer = Some(new_issuer.to_owned());
        config.save()?;
    }

    let tls_config = TlsConfig {
        pin_sha256: Some(new_fingerprint.to_owned()),
        pin_issuer: Some(new_issuer.to_owned()),
        pin_issuer_der: existing_issuer_der,
        server_name: Some(ctx.server_name.clone()),
        ..Default::default()
    };

    detect_and_build_client(ctx, &tls_config, config).await
}

/// Detect server settings and build a client, persisting the detected
/// settings to config. Shared tail logic for TOFU and pin rotation flows.
async fn detect_and_build_client(
    ctx: &ConnectContext,
    tls_config: &TlsConfig,
    config: &mut Config,
) -> Result<BugzillaClient> {
    let settings =
        crate::client::detect_server_settings(&ctx.url, &ctx.api_key, ctx.email_hint(), tls_config)
            .await?;
    persist_detected_settings(config, &ctx.server_name, &settings, true)?;
    let api_mode = ctx.api_override.unwrap_or(settings.api_mode);
    ctx.build_client(settings.auth_method, api_mode, tls_config)
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
async fn classify_and_handle_tls_failure(
    err: &BzrError,
    ctx: &ConnectContext,
    tls_config: &TlsConfig,
    config: &mut Config,
) -> Result<Option<BugzillaClient>> {
    if should_offer_tofu(err, tls_config) {
        let client = handle_tofu(ctx, config).await?;
        return Ok(Some(client));
    }
    if let Some(pin_failure) = crate::tls::pin_failure::classify(err, &ctx.server_name) {
        match pin_failure {
            crate::tls::pin_failure::TlsPinFailure::PinMismatch { error, new_issuer } => {
                let BzrError::PinMismatch {
                    expected, actual, ..
                } = error
                else {
                    unreachable!("pin failure classifier returned a non-pin mismatch error")
                };
                let client =
                    handle_pin_rotation(ctx, &expected, &actual, &new_issuer, config).await?;
                return Ok(Some(client));
            }
            crate::tls::pin_failure::TlsPinFailure::IssuerChanged(error) => return Err(error),
        }
    }
    Ok(None)
}

/// Run `detect_server_settings` and handle TLS errors with TOFU or
/// pin rotation flows as appropriate.
async fn detect_with_tofu_fallback(
    ctx: &ConnectContext,
    tls_config: &TlsConfig,
    config: &mut Config,
) -> Result<DetectOrClient> {
    let err = match crate::client::detect_server_settings(
        &ctx.url,
        &ctx.api_key,
        ctx.email_hint(),
        tls_config,
    )
    .await
    {
        Ok(settings) => return Ok(DetectOrClient::Settings(settings)),
        Err(e) => e,
    };
    match classify_and_handle_tls_failure(&err, ctx, tls_config, config).await? {
        Some(client) => Ok(DetectOrClient::Client(client)),
        None => Err(err),
    }
}

/// Either detected settings (continue normal flow) or a fully-built
/// client (TOFU/rotation handled everything).
enum DetectOrClient {
    Settings(DetectedServerSettings),
    Client(BugzillaClient),
}

/// Connect to a Bugzilla server with auto-configuration.
///
/// On first connection to a server, detects auth method and API mode, then
/// persists these settings to the config file for subsequent connections.
/// The server's configured email (if any) is stored in the client for
/// Bugzilla 5.0 whoami fallback.
///
/// When a TLS certificate error occurs and no trust mechanism is configured,
/// offers an interactive TOFU (trust-on-first-use) prompt. When a pinned
/// certificate has rotated, offers a rotation prompt.
pub async fn connect_and_configure(
    server: Option<&str>,
    api_override: Option<ApiMode>,
) -> Result<BugzillaClient> {
    let mut config = Config::load()?;
    let (server_name, srv) = config.resolve_server(server)?;
    let tls_config = srv.tls_config(server_name);
    let ctx = ConnectContext {
        server_name: server_name.to_string(),
        url: srv.url.clone(),
        api_key: srv.resolve_api_key(server_name)?,
        email: srv.email.clone(),
        api_override,
    };

    if tls_config.insecure {
        tracing::warn!(
            "TLS certificate verification disabled for server '{}'",
            ctx.server_name
        );
    }

    // Three cases: fully cached, partially cached (auth only), or uncached.
    let (auth, resolved_mode) = match (srv.auth_method, srv.api_mode) {
        (Some(method), Some(mode)) => {
            // Even with full cache, surface TLS errors at connect-time so
            // TOFU and pin-rotation prompts can fire. Skipped only when
            // verification is explicitly disabled (`tls_insecure`); for
            // pinned servers and custom-CA servers we still probe so a
            // rotated cert / issuer change is caught here rather than at
            // the first real API call.
            if !tls_config.insecure {
                if let Err(e) = probe_tls(&ctx.url, &tls_config).await {
                    if let Some(client) =
                        classify_and_handle_tls_failure(&e, &ctx, &tls_config, &mut config).await?
                    {
                        return Ok(client);
                    }
                    // Non-TLS transport errors don't block: the actual
                    // command will hit the same condition and report it
                    // with full request context.
                }
            }
            (method, mode)
        }
        (Some(method), None) => {
            tracing::debug!("auth_method cached but api_mode missing; re-detecting");
            match detect_with_tofu_fallback(&ctx, &tls_config, &mut config).await? {
                DetectOrClient::Client(client) => return Ok(client),
                DetectOrClient::Settings(settings) => {
                    persist_detected_settings(&mut config, &ctx.server_name, &settings, false)?;
                    (method, settings.api_mode)
                }
            }
        }
        _ => match detect_with_tofu_fallback(&ctx, &tls_config, &mut config).await? {
            DetectOrClient::Client(client) => return Ok(client),
            DetectOrClient::Settings(settings) => {
                persist_detected_settings(&mut config, &ctx.server_name, &settings, true)?;
                (settings.auth_method, settings.api_mode)
            }
        },
    };

    let api_mode = api_override.unwrap_or(resolved_mode);
    let client = ctx.build_client(auth, api_mode, &tls_config)?;
    Ok(client)
}

#[cfg(test)]
#[path = "shared_tests.rs"]
mod tests;
