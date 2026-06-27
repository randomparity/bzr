use crate::commands::runtime::invocation::CommandContext;
use crate::config::Config;
use crate::error::Result;
use crate::output::result_types::{write_result, ConfigResult};
use crate::output::writers::Writers;

/// Rename a server alias, preserving credentials (including a keychain move
/// when the secret is stored under the default, server-name account).
pub(super) fn handle(
    old: &str,
    new: &str,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    if old == new {
        return Err(crate::error::BzrError::config(
            "new name must differ from the current name",
        ));
    }
    // Advisory snapshot: read unvalidated so an unrelated invalid server does
    // not block the rename.
    let config = Config::read_unvalidated_at(ctx.config_path_override())?;
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

    // `update_locked_without_validation`: a rename preserves the server
    // contents, so avoid blocking on whole-config validation for unrelated
    // invalid entries.
    Config::update_locked_without_validation_at(ctx.config_path_override(), |config| {
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
    let path = Config::path_at(ctx.config_path_override())?;

    let human = format!(
        "Renamed server '{old}' to '{new}'.\nConfig file: {}",
        path.display()
    );
    write_result(
        &ConfigResult::renamed(old, new, path.to_string_lossy()),
        &human,
        ctx.format(),
        w.out,
    );
    Ok(())
}

#[cfg(test)]
#[path = "rename_tests.rs"]
mod tests;
