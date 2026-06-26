//! Connection-target resolution: turning a [`CommandContext`] (inline
//! `--server-url` or named config server) into a [`ConnectContext`] plus its
//! TLS config and any cached auth/mode, and the per-target persistence and
//! client-construction helpers that hang off [`ConnectContext`].

use std::path::{Path, PathBuf};

use crate::client::BugzillaClient;
use crate::client::DetectedServerSettings;
use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::inline_server::{InlineServer, INLINE_SERVER_NAME};
use crate::config::{Config, ServerConfig};
use crate::error::Result;
use crate::tls::TlsConfig;
use crate::types::transport::{ApiMode, AuthMethod};

use super::detect::persist_detected_settings;

pub(super) struct ConnectContext {
    pub(super) server_name: String,
    pub(super) url: String,
    pub(super) api_key: Option<String>,
    pub(super) email: Option<String>,
    pub(super) api_override: Option<ApiMode>,
    pub(super) request_timeout: std::time::Duration,
    pub(super) retry_max: u32,
    pub(super) config_path_override: Option<PathBuf>,
    /// Whether detected settings (and TOFU pins) may be written back to the
    /// config file. `false` for an inline `--server-url` server, which has no
    /// config entry and must leave the filesystem untouched.
    pub(super) persist: bool,
}

impl ConnectContext {
    pub(super) fn email_hint(&self) -> Option<&str> {
        self.email.as_deref()
    }

    /// Persist detected settings to config, unless this is an ephemeral
    /// (inline) server — in which case it is a no-op so a stateless invocation
    /// writes nothing to disk.
    pub(super) fn persist_settings(
        &self,
        settings: &DetectedServerSettings,
        persist_auth: bool,
    ) -> Result<()> {
        if self.persist {
            persist_detected_settings(
                self.config_path_override.as_deref(),
                &self.server_name,
                settings,
                persist_auth,
            )?;
        }
        Ok(())
    }

    /// Apply `mutator` to the config under the lock, unless this is an ephemeral
    /// (inline) server. Routes the TOFU/rotation pin writes through the same
    /// `persist` gate as [`Self::persist_settings`], so "ephemeral ⇒ no config
    /// writes" is a single invariant rather than a flag checked at each site.
    pub(super) fn persist_locked(
        &self,
        mutator: impl FnOnce(&mut Config) -> Result<()>,
    ) -> Result<()> {
        if self.persist {
            Config::update_locked_at(self.config_path_override.as_deref(), mutator)?;
        }
        Ok(())
    }

    pub(super) fn hostname(&self) -> String {
        extract_hostname(&self.url)
    }

    pub(super) fn build_client(
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
            server_name: &self.server_name,
            tls_config,
            request_timeout: self.request_timeout,
            retry_max: self.retry_max,
        })
    }
}

/// Extract the hostname from a URL string, falling back to the raw URL
/// if parsing fails.
pub(super) fn extract_hostname(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(String::from))
        .unwrap_or_else(|| url.to_string())
}

pub(super) fn server_tls_config(server: &ServerConfig, server_name: &str) -> TlsConfig {
    TlsConfig {
        insecure: server.tls_insecure,
        ca_cert_path: server.tls_ca_cert.clone(),
        pin_sha256: server.tls_pin_sha256.clone(),
        pin_issuer_der: server.tls_pin_issuer_der.clone(),
        server_name: Some(server_name.to_string()),
    }
}

/// A resolved connection target: the [`ConnectContext`] plus the TLS config and
/// any cached auth/mode. Produced from either an inline `--server-url`
/// definition or a named config server.
pub(super) struct ConnectTarget {
    pub(super) ctx: ConnectContext,
    pub(super) tls_config: TlsConfig,
    pub(super) cached_auth: Option<AuthMethod>,
    pub(super) cached_mode: Option<ApiMode>,
    pub(super) pin_current_cert: bool,
}

/// Resolve the connection target. When an inline server is set on the command
/// context, builds an ephemeral, never-persisted target and skips the config
/// file entirely — so a fully stateless invocation needs no config. Otherwise
/// loads config and resolves the named (or default) server.
pub(super) fn resolve_connect_target(command: &CommandContext) -> Result<ConnectTarget> {
    let api_override = command.api();
    if let Some(inline) = command.inline_server() {
        return resolve_inline_target(command, inline, api_override);
    }
    resolve_config_target(command, api_override)
}

fn resolve_inline_target(
    command: &CommandContext,
    inline: &InlineServer,
    api_override: Option<ApiMode>,
) -> Result<ConnectTarget> {
    let mut srv = match inline.api_key_env.as_ref() {
        Some(api_key_env) => ServerConfig::from_url_with_env_key(
            inline.url.clone(),
            api_key_env.clone(),
            inline.email.clone(),
        ),
        None => ServerConfig {
            url: inline.url.clone(),
            email: inline.email.clone(),
            ..ServerConfig::default()
        },
    };
    srv.tls_insecure = inline.tls.insecure;
    srv.tls_ca_cert.clone_from(&inline.tls.ca_cert_path);
    srv.tls_pin_sha256.clone_from(&inline.tls.pin_sha256);
    srv.validate(INLINE_SERVER_NAME)?;

    let tls_config = server_tls_config(&srv, INLINE_SERVER_NAME);
    let ctx = ConnectContext {
        server_name: INLINE_SERVER_NAME.to_string(),
        url: srv.url.clone(),
        api_key: crate::credentials::resolve_optional_api_key(&srv, INLINE_SERVER_NAME)?,
        email: srv.email.clone(),
        api_override,
        request_timeout: command.request_timeout(),
        retry_max: command.retry_max(),
        config_path_override: None,
        persist: false,
    };
    Ok(ConnectTarget {
        ctx,
        tls_config,
        cached_auth: None,
        cached_mode: None,
        pin_current_cert: inline.tls.pin_now,
    })
}

fn resolve_config_target(
    command: &CommandContext,
    api_override: Option<ApiMode>,
) -> Result<ConnectTarget> {
    let config = Config::load_at(command.config_path_override())?;
    let (server_name, srv) = config.resolve_server(command.server())?;
    let tls_config = server_tls_config(srv, server_name);
    let api_key = crate::credentials::resolve_optional_api_key(srv, server_name)?;
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
        request_timeout: command.request_timeout(),
        retry_max: command.retry_max(),
        config_path_override: command.config_path_override().map(Path::to_path_buf),
        persist: true,
    };
    Ok(ConnectTarget {
        ctx,
        tls_config,
        cached_auth,
        cached_mode: srv.api_mode,
        pin_current_cert: false,
    })
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
