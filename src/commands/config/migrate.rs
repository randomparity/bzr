use crate::config::Config;
use crate::error::Result;
use crate::output::result_types::{write_result, ConfigResult};
use crate::output::writers::Writers;
use crate::types::OutputFormat;

/// `migrate-to-keyring` operands: which server to migrate and the
/// optional `--service` / `--account` overrides for where the credential
/// gets stored. Bundles together so `handle` stays under the
/// 5-positional-arg threshold; `Option<&str>` defaults
/// (service="bzr", account=name) are resolved inside the function.
#[derive(Clone, Copy)]
pub(super) struct MigrateSpec<'a> {
    pub(super) name: &'a str,
    pub(super) service: Option<&'a str>,
    pub(super) account: Option<&'a str>,
}

pub(super) fn handle(
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
    let Some(source_kind) = source_kind else {
        return Err(crate::error::BzrError::config(format!(
            "server '{name}' has no API key source to migrate"
        )));
    };

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
