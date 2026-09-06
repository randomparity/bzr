use std::fmt::Write as _;
use std::path::PathBuf;

use crate::commands::runtime::invocation::CommandContext;
use crate::config::{Config, ServerConfig};
use crate::error::Result;
use crate::output::result_types::{write_result, ConfigResult};
use crate::output::writers::Writers;

pub(super) struct SetServerArgs<'a> {
    pub(super) name: &'a str,
    pub(super) url: &'a str,
    pub(super) api_key: Option<&'a str>,
    pub(super) api_key_env: Option<&'a str>,
    pub(super) email: Option<&'a str>,
    pub(super) auth_method: Option<crate::types::transport::AuthMethod>,
    pub(super) tls_insecure: bool,
    pub(super) tls_ca_cert: Option<&'a str>,
    pub(super) tls_pin_sha256: Option<&'a str>,
    pub(super) tls_pin_now: bool,
    pub(super) tls_pin_clear: bool,
}

pub(super) async fn handle(
    args: &SetServerArgs<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let SetServerArgs {
        name,
        url,
        api_key,
        api_key_env,
        email,
        auth_method,
        tls_insecure,
        tls_ca_cert,
        tls_pin_sha256,
        tls_pin_now,
        tls_pin_clear,
    } = *args;

    // Handle --tls-pin-clear: clear pinning fields on an existing server.
    if tls_pin_clear {
        Config::update_locked_at(ctx.config_path_override(), |config| {
            let server = config.servers.get_mut(name).ok_or_else(|| {
                crate::error::BzrError::config(format!(
                    "server '{name}' not found — nothing to clear"
                ))
            })?;
            server.tls_pin_sha256 = None;
            server.tls_pin_issuer = None;
            server.tls_pin_issuer_der = None;
            Ok(())
        })?;
        let _ = writeln!(w.err, "Certificate pin cleared for server '{name}'.");
        return Ok(());
    }

    if api_key.is_some() && api_key_env.is_some() {
        return Err(crate::error::BzrError::input(
            "provide at most one of --api-key or --api-key-env".into(),
        ));
    }
    let is_update = Config::load_at(ctx.config_path_override())?
        .servers
        .contains_key(name);
    let mut server_config = ServerConfig {
        url: url.to_owned(),
        api_key: api_key.map(str::to_owned),
        api_key_env: api_key_env.map(str::to_owned),
        api_key_keyring: None,
        email: email.map(str::to_owned),
        auth_method,
        api_mode: None,
        server_version: None,
        server_extensions: None,
        server_extensions_url: None,
        tls_insecure,
        tls_ca_cert: tls_ca_cert.map(PathBuf::from),
        tls_pin_sha256: tls_pin_sha256.map(str::to_owned),
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };

    // Handle --tls-pin-now: probe the server cert and ask user to confirm.
    if tls_pin_now {
        let (fingerprint, issuer, issuer_der) =
            crate::tls::tofu::probe_server_cert(&server_config.url, ctx.request_timeout()).await?;
        let _ = writeln!(w.err, "Certificate fingerprint: {fingerprint}");
        let _ = writeln!(w.err, "Issuer:                  {issuer}");
        let confirmed = crate::tls::tofu::confirm_pin()?;
        if confirmed {
            server_config.tls_pin_sha256 = Some(fingerprint);
            server_config.tls_pin_issuer = Some(issuer);
            server_config.tls_pin_issuer_der = issuer_der;
        } else {
            return Err(crate::error::BzrError::config(
                "certificate pinning cancelled by user".to_owned(),
            ));
        }
    }

    let updated = Config::update_locked_at(ctx.config_path_override(), move |config| {
        config.servers.insert(name.to_owned(), server_config);
        if config.default_server.is_none() {
            config.default_server = Some(name.to_owned());
        }
        Ok(())
    })?;
    let is_default = updated.default_server.as_deref() == Some(name);
    let path = Config::path_at(ctx.config_path_override())?;

    let verb = if is_update { "updated" } else { "configured" };
    let mut human = format!("Server '{name}' {verb} at {url}");
    if is_default {
        human.push_str("\nSet as default server.");
    }
    if let Some(var_name) = api_key_env {
        let _ = write!(human, "\nAPI key source: env var {var_name}");
    } else if api_key.is_some() {
        human.push_str("\nAPI key source: inline config value");
    } else {
        human.push_str("\nAPI key source: none (read-only)");
    }
    let _ = write!(human, "\nConfig file: {}", path.display());

    write_result(
        &ConfigResult::configured(name, url, is_default, path.to_string_lossy(), is_update),
        &human,
        ctx.format(),
        w.out,
    );
    Ok(())
}

#[cfg(test)]
#[path = "set_server_tests.rs"]
mod tests;
