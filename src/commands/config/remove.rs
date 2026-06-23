use crate::config::Config;
use crate::error::Result;
use crate::output::result_types::{write_result, ConfigResult};
use crate::output::writers::Writers;
use crate::types::OutputFormat;

/// Remove a server alias from the config, dropping any keychain entry.
pub(super) fn handle(name: &str, format: OutputFormat, w: &mut Writers<'_>) -> Result<()> {
    // Advisory snapshot: read unvalidated so an unrelated invalid server does
    // not block the removal.
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

    // `update_locked_without_validation`: removal cannot improve or worsen an
    // unrelated invalid server, so avoid blocking on whole-config validation.
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
