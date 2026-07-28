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
    if !config.servers.contains_key(name) {
        return Err(crate::error::BzrError::config(format!(
            "server '{name}' not found"
        )));
    }

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

    // `update_locked_without_validation`: removal cannot improve or worsen an
    // unrelated invalid server, so avoid blocking on whole-config validation.
    //
    // Resolve the keychain coordinates from the config the mutator sees, not
    // from the advisory snapshot above: the snapshot is taken outside the lock,
    // so a concurrent `set-keyring` between the two would otherwise delete the
    // *old* entry and orphan the secret the removed server actually pointed at.
    let mut keyring_entry: Option<(String, String)> = None;
    Config::update_locked_without_validation_at(ctx.config_path_override(), |config| {
        keyring_entry = config.servers.remove(name).and_then(|removed| {
            removed.api_key_keyring.map(|keyring_ref| {
                (
                    keyring_ref.service_or_default().to_string(),
                    keyring_ref.account_or_default(name).to_string(),
                )
            })
        });
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
