use crate::commands::runtime::invocation::CommandContext;
use crate::config::Config;
use crate::error::Result;
use crate::output::result_types::{write_result, ConfigResult};
use crate::output::writers::Writers;

/// Remove a server alias from the config, dropping any keychain entry.
pub(super) fn handle(name: &str, ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    // Advisory snapshot: read unvalidated so an unrelated invalid server does
    // not block the removal.
    let config = Config::read_unvalidated_at(ctx.config_path_override())?;
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

    let keyring_entry = server.api_key_keyring.as_ref().map(|keyring_ref| {
        (
            keyring_ref.service_or_default().to_string(),
            keyring_ref.account_or_default(name).to_string(),
        )
    });

    // `update_locked_without_validation`: removal cannot improve or worsen an
    // unrelated invalid server, so avoid blocking on whole-config validation.
    Config::update_locked_without_validation_at(ctx.config_path_override(), |config| {
        config.servers.remove(name);
        if config.default_server.as_deref() == Some(name) {
            config.default_server = None;
        }
        Ok(())
    })?;

    // Drop the keychain entry only after the config write commits, so a failed
    // write never destroys the secret for a server that is still configured.
    // The reverse order loses the credential outright; this order can at worst
    // orphan an unreferenced entry. (Same ordering as `config rename-server`.)
    // Idempotent: a missing entry is not an error.
    if let Some((service, account)) = keyring_entry {
        crate::credentials::keyring::delete(&service, &account)?;
    }

    let path = Config::path_at(ctx.config_path_override())?;

    let human = format!("Removed server '{name}'.\nConfig file: {}", path.display());
    write_result(
        &ConfigResult::removed(name, path.to_string_lossy()),
        &human,
        ctx.format(),
        w.out,
    );
    Ok(())
}

#[cfg(test)]
#[path = "remove_tests.rs"]
mod tests;
