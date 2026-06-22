use crate::client::BugzillaClient;
use crate::client::DetectedServerSettings;
use crate::config::{Config, ServerConfig};
use crate::error::{BzrError, Result};
use crate::tls::TlsConfig;
use crate::types::{ApiMode, AuthMethod};

/// Persist detected server settings to config under the lock.
/// Persists `auth_method` when `persist_auth` is true and detection produced
/// one. Only persists `api_mode`/`server_version` when version detection
/// succeeded.
///
/// If the server was concurrently removed from disk, this is a no-op (we do
/// not resurrect a deleted server with detected settings) — logged, not silent.
fn persist_detected_settings(
    server_name: &str,
    settings: &DetectedServerSettings,
    persist_auth: bool,
) -> Result<()> {
    Config::update_locked(|config| {
        let Some(srv) = config.servers.get_mut(server_name) else {
            tracing::debug!(
                "server '{server_name}' no longer in config; skipping settings persist"
            );
            return Ok(());
        };
        if persist_auth {
            if let Some(auth_method) = settings.auth_method {
                srv.auth_method = Some(auth_method);
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
    api_key: Option<String>,
    email: Option<String>,
    api_override: Option<ApiMode>,
    /// Whether detected settings (and TOFU pins) may be written back to the
    /// config file. `false` for an inline `--server-url` server, which has no
    /// config entry and must leave the filesystem untouched.
    persist: bool,
}

impl ConnectContext {
    fn email_hint(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Persist detected settings to config, unless this is an ephemeral
    /// (inline) server — in which case it is a no-op so a stateless invocation
    /// writes nothing to disk.
    fn persist_settings(
        &self,
        settings: &DetectedServerSettings,
        persist_auth: bool,
    ) -> Result<()> {
        if self.persist {
            persist_detected_settings(&self.server_name, settings, persist_auth)?;
        }
        Ok(())
    }

    /// Apply `mutator` to the config under the lock, unless this is an ephemeral
    /// (inline) server. Routes the TOFU/rotation pin writes through the same
    /// `persist` gate as [`Self::persist_settings`], so "ephemeral ⇒ no config
    /// writes" is a single invariant rather than a flag checked at each site.
    fn persist_locked(&self, mutator: impl FnOnce(&mut Config) -> Result<()>) -> Result<()> {
        if self.persist {
            Config::update_locked(mutator)?;
        }
        Ok(())
    }

    fn hostname(&self) -> String {
        extract_hostname(&self.url)
    }

    fn build_client(
        &self,
        auth_method: Option<AuthMethod>,
        api_mode: ApiMode,
        tls_config: &TlsConfig,
    ) -> Result<BugzillaClient> {
        BugzillaClient::new(crate::client::BugzillaClientConfig {
            base_url: &self.url,
            credential: self.api_key.as_deref(),
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
async fn handle_tofu(ctx: &ConnectContext) -> Result<BugzillaClient> {
    let hostname = ctx.hostname();
    let (fingerprint, issuer, issuer_der) = crate::tls::tofu::probe_server_cert(&ctx.url).await?;

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
async fn handle_pin_rotation(
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
    let existing_issuer_der = Config::load().ok().and_then(|c| {
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

/// Detect server settings and build a client, persisting the detected
/// settings to config. Shared tail logic for TOFU and pin rotation flows.
async fn detect_and_build_client(
    ctx: &ConnectContext,
    tls_config: &TlsConfig,
) -> Result<BugzillaClient> {
    let settings = detect_settings(ctx, tls_config).await?;
    ctx.persist_settings(&settings, true)?;
    let api_mode = ctx.api_override.unwrap_or(settings.api_mode);
    ctx.build_client(settings.auth_method, api_mode, tls_config)
}

async fn detect_settings(
    ctx: &ConnectContext,
    tls_config: &TlsConfig,
) -> Result<DetectedServerSettings> {
    if let Some(api_key) = ctx.api_key.as_deref() {
        crate::client::detect_server_settings(&ctx.url, api_key, ctx.email_hint(), tls_config).await
    } else {
        crate::client::detect_server_settings_without_auth(&ctx.url, tls_config).await
    }
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

/// Run `detect_server_settings` and handle TLS errors with TOFU or
/// pin rotation flows as appropriate.
async fn detect_with_tofu_fallback(
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
enum DetectOrClient {
    Settings(DetectedServerSettings),
    Client(BugzillaClient),
}

/// A resolved connection target: the [`ConnectContext`] plus the TLS config and
/// any cached auth/mode. Produced from either an inline `--server-url`
/// definition or a named config server.
struct ConnectTarget {
    ctx: ConnectContext,
    tls_config: TlsConfig,
    cached_auth: Option<AuthMethod>,
    cached_mode: Option<ApiMode>,
}

/// Resolve the connection target. When an inline server is set (global
/// `--server-url`), builds an ephemeral, never-persisted target and skips the
/// config file entirely — so a fully stateless invocation needs no config.
/// Otherwise loads config and resolves the named (or default) server.
fn resolve_connect_target(
    server: Option<&str>,
    api_override: Option<ApiMode>,
) -> Result<ConnectTarget> {
    if let Some(inline) = crate::commands::runtime::inline_server::get() {
        let name = crate::commands::runtime::inline_server::INLINE_SERVER_NAME;
        let srv = match inline.api_key_env {
            Some(api_key_env) => {
                ServerConfig::from_url_with_env_key(inline.url, api_key_env, inline.email)
            }
            None => ServerConfig {
                url: inline.url,
                email: inline.email,
                ..ServerConfig::default()
            },
        };
        srv.validate(name)?;
        let tls_config = srv.tls_config(name);
        let ctx = ConnectContext {
            server_name: name.to_string(),
            url: srv.url.clone(),
            api_key: srv.resolve_optional_api_key(name)?,
            email: srv.email.clone(),
            api_override,
            persist: false,
        };
        return Ok(ConnectTarget {
            ctx,
            tls_config,
            cached_auth: None,
            cached_mode: None,
        });
    }

    let config = Config::load()?;
    let (server_name, srv) = config.resolve_server(server)?;
    let tls_config = srv.tls_config(server_name);
    let api_key = srv.resolve_optional_api_key(server_name)?;
    let cached_auth = if api_key.is_some() {
        srv.auth_method
    } else {
        None
    };
    let ctx = ConnectContext {
        server_name: server_name.to_string(),
        url: srv.url.clone(),
        api_key,
        email: srv.email.clone(),
        api_override,
        persist: true,
    };
    Ok(ConnectTarget {
        ctx,
        tls_config,
        cached_auth,
        cached_mode: srv.api_mode,
    })
}

async fn probe_cached_connection(
    ctx: &ConnectContext,
    tls_config: &TlsConfig,
) -> Result<Option<BugzillaClient>> {
    if tls_config.insecure {
        return Ok(None);
    }

    if let Err(e) = probe_tls(&ctx.url, tls_config).await {
        if let Some(client) = classify_and_handle_tls_failure(&e, ctx, tls_config).await? {
            return Ok(Some(client));
        }
        // Non-TLS transport errors don't block: the actual command will hit
        // the same condition and report it with full request context.
    }

    Ok(None)
}

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
pub async fn connect_and_configure(
    server: Option<&str>,
    api_override: Option<ApiMode>,
) -> Result<BugzillaClient> {
    let ConnectTarget {
        ctx,
        tls_config,
        cached_auth,
        cached_mode,
    } = resolve_connect_target(server, api_override)?;

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

    let api_mode = api_override.unwrap_or(resolved_mode);
    let client = ctx.build_client(auth, api_mode, &tls_config)?;
    Ok(client)
}

/// Where a body string comes from. Pure result of classifying the
/// inline + file flag pair; carries no I/O.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BodySource {
    /// A literal inline value (`--body "text"`).
    Literal(String),
    /// Read all of stdin (`--body -` or `--*-file -`).
    Stdin,
    /// Read a UTF-8 file (`--*-file PATH`, PATH != "-").
    File(std::path::PathBuf),
    /// Neither source supplied; the caller applies its own fallback.
    None,
}

/// Classify an inline flag value and/or a `--*-file` path into a
/// [`BodySource`], honouring the `-` = stdin convention for both. The two
/// sources are mutually exclusive; clap `conflicts_with` makes the
/// both-present case unreachable in normal use, but this guards regardless so
/// correctness does not depend on the CLI layer. `inline_flag` / `file_flag`
/// name the originating options for the mutual-exclusion error message.
pub(crate) fn classify_body_source(
    inline: Option<&str>,
    file: Option<&std::path::Path>,
    inline_flag: &str,
    file_flag: &str,
) -> Result<BodySource> {
    match (inline, file) {
        (Some(_), Some(_)) => Err(BzrError::InputValidation(format!(
            "{inline_flag} and {file_flag} are mutually exclusive"
        ))),
        (Some("-"), None) => Ok(BodySource::Stdin),
        (Some(s), None) => Ok(BodySource::Literal(s.to_string())),
        (None, Some(path)) => {
            if path == std::path::Path::new("-") {
                Ok(BodySource::Stdin)
            } else {
                Ok(BodySource::File(path.to_path_buf()))
            }
        }
        (None, None) => Ok(BodySource::None),
    }
}

/// Read everything from `reader` into a `String`, mapping I/O failures
/// (including non-UTF-8 input) to a `BzrError`.
fn read_to_string_from(reader: &mut impl std::io::Read) -> Result<String> {
    let mut buf = String::new();
    reader.read_to_string(&mut buf)?;
    Ok(buf)
}

/// Read all of stdin to a `String`. Shared by the `-` (dash) convention and
/// the flag-omitted auto-stdin paths.
pub(crate) fn read_stdin_to_string() -> Result<String> {
    read_to_string_from(&mut std::io::stdin().lock())
}

/// Turn a classified [`BodySource`] into the actual body, or `Ok(None)` for
/// [`BodySource::None`] so the caller applies its own fallback. This is the
/// only place that performs stdin/file I/O for the explicit body flags.
pub(crate) fn materialize_body_source(
    source: BodySource,
    file_flag: &str,
) -> Result<Option<String>> {
    match source {
        BodySource::Literal(s) => Ok(Some(s)),
        BodySource::Stdin => Ok(Some(read_stdin_to_string()?)),
        BodySource::File(path) => Ok(Some(read_file_with_context(&path, file_flag)?)),
        BodySource::None => Ok(None),
    }
}

/// Read a UTF-8 file, mapping any I/O error to an `InputValidation` error that
/// names the originating CLI flag and the path. `flag` is the user-facing
/// option name (e.g. `--description-file`) used to prefix the message.
pub(crate) fn read_file_with_context(path: &std::path::Path, flag: &str) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| {
        BzrError::InputValidation(format!(
            "{flag} could not be read ({}): {e}",
            path.display()
        ))
    })
}

/// Overwrite `target` with `value` when an `Option` flag was supplied; an
/// absent flag (`None`) leaves the existing value unchanged. Returns `true` if
/// the flag was supplied (i.e. an assignment happened). Used by the in-place
/// `update` merges for templates and saved queries.
pub(crate) fn merge_set(target: &mut Option<String>, value: Option<&str>) -> bool {
    if let Some(v) = value {
        *target = Some(v.to_owned());
        true
    } else {
        false
    }
}

/// Overwrite `target` with `value` when a repeatable flag was supplied (a
/// non-empty `Vec`); an empty `Vec` (absent flag) leaves it unchanged. Returns
/// `true` if the flag was supplied.
pub(crate) fn merge_vec(target: &mut Vec<String>, value: &[String]) -> bool {
    if value.is_empty() {
        false
    } else {
        *target = value.to_vec();
        true
    }
}

#[cfg(test)]
#[path = "shared_tests.rs"]
mod tests;
