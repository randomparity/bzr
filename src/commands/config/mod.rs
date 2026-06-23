//! Configuration management commands.
//!
//! Config operations are pure local file I/O — no network client or auth
//! detection needed. The function is async for signature consistency with
//! sibling command modules.

use crate::cli::ConfigAction;
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::writers::Writers;

mod keyring;
mod migrate;
mod remove;
mod rename;
mod set_default;
mod set_server;
mod show;

#[cfg(test)]
use crate::config::{Config, ServerConfig};
#[cfg(test)]
use crate::types::OutputFormat;

pub async fn execute(
    action: &ConfigAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        ConfigAction::SetServer {
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
        } => {
            set_server::handle(
                &set_server::SetServerArgs {
                    name: name.as_str(),
                    url: url.as_str(),
                    api_key: api_key.as_deref(),
                    api_key_env: api_key_env.as_deref(),
                    email: email.as_deref(),
                    auth_method: *auth_method,
                    tls_insecure: *tls_insecure,
                    tls_ca_cert: tls_ca_cert.as_deref(),
                    tls_pin_sha256: tls_pin_sha256.as_deref(),
                    tls_pin_now: *tls_pin_now,
                    tls_pin_clear: *tls_pin_clear,
                },
                ctx,
                w,
            )
            .await
        }
        ConfigAction::SetDefault { name } => set_default::handle(name.as_str(), ctx, w),
        ConfigAction::Show => show::handle(ctx, w),
        ConfigAction::SetKeyring {
            name,
            service,
            account,
        } => keyring::set(name, service.as_deref(), account.as_deref(), ctx, w),
        ConfigAction::UnsetKeyring { name } => keyring::unset(name.as_str(), ctx, w),
        ConfigAction::MigrateToKeyring {
            name,
            service,
            account,
            yes,
        } => {
            let spec = migrate::MigrateSpec {
                name: name.as_str(),
                service: service.as_deref(),
                account: account.as_deref(),
            };
            migrate::handle(spec, *yes, ctx, w)
        }
        ConfigAction::RemoveServer { name } => remove::handle(name.as_str(), ctx, w),
        ConfigAction::RenameServer { old, new } => {
            rename::handle(old.as_str(), new.as_str(), ctx, w)
        }
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
