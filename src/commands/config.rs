//! Configuration management commands.
//!
//! Config operations are pure local file I/O — no network client or auth
//! detection needed. The function is async for signature consistency with
//! sibling command modules.

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::cli::ConfigAction;
use crate::config::{Config, ServerConfig};
use crate::error::Result;
use crate::output::resources::config::{write_config, ConfigView};
use crate::output::result_types::{write_result, ConfigResult};
use crate::output::writers::Writers;
use crate::types::OutputFormat;

pub async fn execute(
    action: &ConfigAction,
    _server: Option<&str>,
    format: OutputFormat,
    _api: Option<crate::types::ApiMode>,
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
            set_server(
                &SetServerArgs {
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
                format,
                w,
            )
            .await
        }
        ConfigAction::SetDefault { name } => {
            Config::update_locked(|config| {
                if !config.servers.contains_key(name) {
                    return Err(crate::error::BzrError::config(format!(
                        "server '{name}' not found"
                    )));
                }
                config.default_server = Some(name.clone());
                Ok(())
            })?;
            let path = Config::path()?;

            write_result(
                &ConfigResult::default_set(name.as_str(), path.to_string_lossy()),
                &format!(
                    "Default server set to '{name}'\nConfig file: {}",
                    path.display()
                ),
                format,
                w.out,
            );
            Ok(())
        }
        ConfigAction::Show => {
            let config = Config::load()?;
            let path = Config::path()?;
            let view = ConfigView::from_config(&config, &path);
            write_config(&view, format, w.out);
            Ok(())
        }
        ConfigAction::SetKeyring {
            name,
            service,
            account,
        } => set_keyring(name, service.as_deref(), account.as_deref(), format, w),
        ConfigAction::UnsetKeyring { name } => unset_keyring(name.as_str(), format, w),
        ConfigAction::MigrateToKeyring {
            name,
            service,
            account,
            yes,
        } => {
            let spec = MigrateSpec {
                name: name.as_str(),
                service: service.as_deref(),
                account: account.as_deref(),
            };
            migrate_to_keyring(spec, *yes, format, w)
        }
        ConfigAction::RemoveServer { name } => remove_server(name.as_str(), format, w),
        ConfigAction::RenameServer { old, new } => {
            rename_server(old.as_str(), new.as_str(), format, w)
        }
    }
}

/// Remove a server alias from the config, dropping any keychain entry.
fn remove_server(name: &str, format: OutputFormat, w: &mut Writers<'_>) -> Result<()> {
    // Advisory snapshot: read unvalidated so an unrelated credential-less
    // server (left by `unset-keyring`) does not block the removal.
    let config = Config::read_unvalidated()?;
    let server = config
        .servers
        .get(name)
        .ok_or_else(|| crate::error::BzrError::config(format!("server '{name}' not found")))?;

    // Refuse to remove the current default while other servers remain — that
    // would silently leave bzr with no default. Removing the only server is
    // allowed and clears the pointer.
    let is_default = config.default_server.as_deref() == Some(name);
    if is_default && config.servers.len() > 1 {
        return Err(crate::error::BzrError::config(format!(
            "server '{name}' is the current default; set a different default first \
             with `bzr config set-default <name>` before removing it"
        )));
    }

    // Drop the keychain entry if the server kept its key there (idempotent —
    // a missing entry is not an error).
    if let Some(keyring_ref) = server.api_key_keyring.as_ref() {
        let service = keyring_ref.service_or_default().to_string();
        let account = keyring_ref.account_or_default(name).to_string();
        crate::credentials::keyring::delete(&service, &account)?;
    }

    // `update_locked_without_validation`: removal never introduces a
    // credential-less server, and the whole-config validator would otherwise
    // reject the write whenever any *other* server is credential-less (the
    // state `unset-keyring` deliberately leaves on disk).
    Config::update_locked_without_validation(|config| {
        config.servers.remove(name);
        if config.default_server.as_deref() == Some(name) {
            config.default_server = None;
        }
        Ok(())
    })?;
    let path = Config::path()?;

    let human = format!("Removed server '{name}'.\nConfig file: {}", path.display());
    write_result(
        &ConfigResult::removed(name, path.to_string_lossy()),
        &human,
        format,
        w.out,
    );
    Ok(())
}

/// Rename a server alias, preserving credentials (including a keychain move
/// when the secret is stored under the default, server-name account).
fn rename_server(old: &str, new: &str, format: OutputFormat, w: &mut Writers<'_>) -> Result<()> {
    if old == new {
        return Err(crate::error::BzrError::config(
            "new name must differ from the current name",
        ));
    }
    // Advisory snapshot: read unvalidated so an unrelated credential-less
    // server (left by `unset-keyring`) does not block the rename.
    let config = Config::read_unvalidated()?;
    let server = config
        .servers
        .get(old)
        .ok_or_else(|| crate::error::BzrError::config(format!("server '{old}' not found")))?;
    if config.servers.contains_key(new) {
        return Err(crate::error::BzrError::config(format!(
            "server '{new}' already exists; choose a different name or remove it first"
        )));
    }

    // When the key lives in the keychain under the *default* account (the
    // server name), move the secret to the new account so the rename does not
    // orphan it. Retrieve+store happen before the config write; the old
    // entry is deleted only after the rename commits, so a failure anywhere
    // never loses the secret.
    let keyring_move = match server.api_key_keyring.as_ref() {
        Some(kr) if kr.account.is_none() => {
            let service = kr.service_or_default().to_string();
            let secret = crate::credentials::keyring::retrieve(&service, old)?;
            crate::credentials::keyring::store(&service, new, &secret)?;
            Some(service)
        }
        _ => None,
    };

    // `update_locked_without_validation`: a rename preserves the credential
    // source, so it cannot create a credential-less server, but the validator
    // would reject the write if any *other* server is already credential-less.
    Config::update_locked_without_validation(|config| {
        let server_cfg = config
            .servers
            .remove(old)
            .ok_or_else(|| crate::error::BzrError::config(format!("server '{old}' disappeared")))?;
        config.servers.insert(new.to_owned(), server_cfg);
        if config.default_server.as_deref() == Some(old) {
            config.default_server = Some(new.to_owned());
        }
        Ok(())
    })?;

    if let Some(service) = keyring_move {
        crate::credentials::keyring::delete(&service, old)?;
    }
    let path = Config::path()?;

    let human = format!(
        "Renamed server '{old}' to '{new}'.\nConfig file: {}",
        path.display()
    );
    write_result(
        &ConfigResult::renamed(old, new, path.to_string_lossy()),
        &human,
        format,
        w.out,
    );
    Ok(())
}

struct SetServerArgs<'a> {
    name: &'a str,
    url: &'a str,
    api_key: Option<&'a str>,
    api_key_env: Option<&'a str>,
    email: Option<&'a str>,
    auth_method: Option<crate::types::AuthMethod>,
    tls_insecure: bool,
    tls_ca_cert: Option<&'a str>,
    tls_pin_sha256: Option<&'a str>,
    tls_pin_now: bool,
    tls_pin_clear: bool,
}

async fn set_server(
    args: &SetServerArgs<'_>,
    format: OutputFormat,
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
        Config::update_locked(|config| {
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

    if api_key.is_some() == api_key_env.is_some() {
        return Err(crate::error::BzrError::InputValidation(
            "provide exactly one of --api-key or --api-key-env".into(),
        ));
    }
    let is_update = Config::load()?.servers.contains_key(name);
    let mut server_config = ServerConfig {
        url: url.to_owned(),
        api_key: api_key.map(str::to_owned),
        api_key_env: api_key_env.map(str::to_owned),
        api_key_keyring: None,
        email: email.map(str::to_owned),
        auth_method,
        api_mode: None,
        server_version: None,
        tls_insecure,
        tls_ca_cert: tls_ca_cert.map(PathBuf::from),
        tls_pin_sha256: tls_pin_sha256.map(str::to_owned),
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };

    // Handle --tls-pin-now: probe the server cert and ask user to confirm.
    if tls_pin_now {
        let (fingerprint, issuer, issuer_der) =
            crate::tls::tofu::probe_server_cert(&server_config.url).await?;
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

    let updated = Config::update_locked(move |config| {
        config.servers.insert(name.to_owned(), server_config);
        if config.default_server.is_none() {
            config.default_server = Some(name.to_owned());
        }
        Ok(())
    })?;
    let is_default = updated.default_server.as_deref() == Some(name);
    let path = Config::path()?;

    let verb = if is_update { "updated" } else { "configured" };
    let mut human = format!("Server '{name}' {verb} at {url}");
    if is_default {
        human.push_str("\nSet as default server.");
    }
    if let Some(var_name) = api_key_env {
        let _ = write!(human, "\nAPI key source: env var {var_name}");
    } else {
        human.push_str("\nAPI key source: inline config value");
    }
    let _ = write!(human, "\nConfig file: {}", path.display());

    write_result(
        &ConfigResult::configured(name, url, is_default, path.to_string_lossy(), is_update),
        &human,
        format,
        w.out,
    );
    Ok(())
}

fn set_keyring(
    name: &str,
    service: Option<&str>,
    account: Option<&str>,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    // Advisory existence check FIRST — lockless, so a nonexistent server is
    // rejected before prompting / writing to the keychain.
    let config = Config::load()?;
    if !config.servers.contains_key(name) {
        return Err(crate::error::BzrError::config(format!(
            "server '{name}' not found; create it first with `bzr config set-server`"
        )));
    }

    let service_name = service.unwrap_or("bzr").to_string();
    let account_name = account.unwrap_or(name).to_string();

    let secret = read_secret_from_prompt_or_env(&service_name, &account_name)?;
    crate::credentials::keyring::store(&service_name, &account_name, &secret)?;

    // Capture raw optional args before the closure (preserve None=default).
    let service_persist = service.map(str::to_owned);
    let account_persist = account.map(str::to_owned);
    let updated = Config::update_locked(move |config| {
        let server = config.servers.get_mut(name).ok_or_else(|| {
            crate::error::BzrError::config(format!("server '{name}' disappeared"))
        })?;
        server.api_key = None;
        server.api_key_env = None;
        server.api_key_keyring = Some(crate::config::KeyringRef {
            service: service_persist,
            account: account_persist,
        });
        Ok(())
    })?;
    let server_url = updated
        .servers
        .get(name)
        .map(|s| s.url.clone())
        .unwrap_or_default();
    let path = Config::path()?;

    let human = format!(
        "Stored API key for server '{name}' in OS keychain \
         (service={service_name}, account={account_name})\nConfig file: {}",
        path.display()
    );
    write_result(
        &ConfigResult::configured(name, &server_url, false, path.to_string_lossy(), true),
        &human,
        format,
        w.out,
    );
    Ok(())
}

fn unset_keyring(name: &str, format: OutputFormat, w: &mut Writers<'_>) -> Result<()> {
    let config = Config::load()?;
    let server = config
        .servers
        .get(name)
        .ok_or_else(|| crate::error::BzrError::config(format!("server '{name}' not found")))?;
    let server_url = server.url.clone();
    let keyring_ref = server.api_key_keyring.as_ref().ok_or_else(|| {
        crate::error::BzrError::config(format!(
            "server '{name}' has no keyring credential to unset"
        ))
    })?;
    let service_name = keyring_ref.service_or_default().to_string();
    let account_name = keyring_ref.account_or_default(name).to_string();
    // Idempotent: missing entry is not an error.
    crate::credentials::keyring::delete(&service_name, &account_name)?;

    // Saving normally would fail validation (the server has no credential
    // source now), but the on-disk hardening (0o600/0o700) must still apply.
    Config::update_locked_without_validation(|config| {
        let server = config
            .servers
            .get_mut(name)
            .ok_or_else(|| crate::error::BzrError::config(format!("server '{name}' not found")))?;
        server.api_key_keyring = None;
        Ok(())
    })?;
    let path = Config::path()?;

    let human = format!(
        "Removed keychain entry for server '{name}' (service={service_name}, \
         account={account_name}).\nThe server entry is still present but has \
         no API key source — re-run `bzr config set-server` or \
         `bzr config set-keyring` to re-credential.\nConfig file: {}",
        path.display()
    );
    write_result(
        &ConfigResult::configured(name, &server_url, false, path.to_string_lossy(), true),
        &human,
        format,
        w.out,
    );
    Ok(())
}

/// `migrate-to-keyring` operands: which server to migrate and the
/// optional `--service` / `--account` overrides for where the credential
/// gets stored. Bundles together so `migrate_to_keyring` stays under
/// the 5-positional-arg threshold; `Option<&str>` defaults
/// (service="bzr", account=name) are resolved inside the function.
#[derive(Clone, Copy)]
struct MigrateSpec<'a> {
    name: &'a str,
    service: Option<&'a str>,
    account: Option<&'a str>,
}

fn migrate_to_keyring(
    spec: MigrateSpec<'_>,
    yes: bool,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    if !yes {
        return Err(crate::error::BzrError::InputValidation(
            "migrate-to-keyring requires --yes to confirm non-interactive migration".into(),
        ));
    }

    let MigrateSpec {
        name,
        service,
        account,
    } = spec;

    let config = Config::load()?;
    let server = config
        .servers
        .get(name)
        .ok_or_else(|| crate::error::BzrError::config(format!("server '{name}' not found")))?;
    let source_kind = server.credential_source_kind()?;
    let server_url = server.url.clone();

    // Refuse migration from an already-keyring source BEFORE writing
    // to the keychain — otherwise we would silently store to an
    // unintended location when --service/--account differ from the
    // existing ref.
    if source_kind == crate::config::CredentialSourceKind::Keyring {
        return Err(crate::error::BzrError::config(format!(
            "server '{name}' already uses a keyring credential source"
        )));
    }

    let current_secret = server.resolve_api_key(name)?;

    let service_name = service.unwrap_or("bzr").to_string();
    let account_name = account.unwrap_or(name).to_string();
    crate::credentials::keyring::store(&service_name, &account_name, &current_secret)?;

    let path = Config::path()?;
    let human = if source_kind == crate::config::CredentialSourceKind::Inline {
        let service_persist = service.map(str::to_owned);
        let account_persist = account.map(str::to_owned);
        Config::update_locked(move |config| {
            let server = config.servers.get_mut(name).ok_or_else(|| {
                crate::error::BzrError::config(format!("server '{name}' disappeared"))
            })?;
            server.api_key = None;
            server.api_key_keyring = Some(crate::config::KeyringRef {
                service: service_persist,
                account: account_persist,
            });
            Ok(())
        })?;
        format!(
            "Migrated server '{name}' from inline API key to OS keychain \
             (service={service_name}, account={account_name}).\nConfig file: {}",
            path.display()
        )
    } else {
        // Env source: store the secret but leave config.toml unchanged.
        format!(
            "Stored API key for server '{name}' in OS keychain \
             (service={service_name}, account={account_name}).\n\
             The server is still configured to read 'api_key_env'. \
             Edit config.toml manually to switch to the keychain if desired; \
             the env var may be shared with other tools.\nConfig file: {}",
            path.display()
        )
    };

    write_result(
        &ConfigResult::configured(name, &server_url, false, path.to_string_lossy(), true),
        &human,
        format,
        w.out,
    );
    Ok(())
}

#[cfg(feature = "keyring")]
fn read_secret_from_prompt_or_env(service: &str, account: &str) -> crate::error::Result<String> {
    // Test hook: integration/unit tests and the functional-test shell
    // script inject the secret via env var so they don't need an
    // interactive TTY. Gated on debug_assertions so release binaries
    // always go through the stdin prompt — the env var cannot be used
    // to bypass prompts in production.
    #[cfg(debug_assertions)]
    {
        if let Ok(val) = std::env::var("BZR_KEYRING_TEST_SECRET") {
            if !val.is_empty() {
                tracing::warn!(
                    "BZR_KEYRING_TEST_SECRET env var is set; using its value \
                     instead of prompting. This hook is only available in \
                     debug builds."
                );
                return Ok(val);
            }
        }
    }

    let prompt =
        format!("Enter API key for service='{service}' account='{account}' (input hidden): ");
    rpassword::prompt_password(&prompt).map_err(|e| {
        crate::error::BzrError::Io(std::io::Error::other(format!(
            "failed to read API key from stdin: {e}"
        )))
    })
}

// Mutation testing: cfg-gated dead code on the default-feature Linux
// test platform. Skipped here rather than via mutants.toml so the live
// keyring version above stays in the test set.
#[cfg(not(feature = "keyring"))]
#[cfg_attr(test, mutants::skip)]
fn read_secret_from_prompt_or_env(_service: &str, _account: &str) -> crate::error::Result<String> {
    Err(crate::error::BzrError::Keyring(
        "this bzr build was compiled without keyring support; \
         rebuild with --features keyring or use api_key_env"
            .into(),
    ))
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
