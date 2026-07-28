use crate::commands::runtime::invocation::CommandContext;
use crate::config::Config;
use crate::error::Result;
use crate::output::result_types::{write_result, ConfigResult};
use crate::output::writers::Writers;

pub(super) fn set(
    name: &str,
    service: Option<&str>,
    account: Option<&str>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    // Advisory existence check FIRST — lockless, so a nonexistent server is
    // rejected before prompting / writing to the keychain.
    //
    // Deliberately a *validating* read, unlike `unset` below. Adding a
    // credential is an administrative operation, not a repair one: it can
    // introduce new structural errors, so failing early on an already-broken
    // config is correct. Do not "unify" these two paths — see
    // `set_keyring_is_blocked_by_other_structurally_invalid_server`.
    let config = Config::load_at(ctx.config_path_override())?;
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
    let updated = Config::update_locked_at(ctx.config_path_override(), move |config| {
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
    let path = Config::path_at(ctx.config_path_override())?;

    let human = format!(
        "Stored API key for server '{name}' in OS keychain \
         (service={service_name}, account={account_name})\nConfig file: {}",
        path.display()
    );
    write_result(
        &ConfigResult::configured(name, &server_url, false, path.to_string_lossy(), true),
        &human,
        ctx.format(),
        w.out,
    );
    Ok(())
}

pub(super) fn unset(name: &str, ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    // Everything is decided inside the mutator, against the config read under
    // the advisory lock. There is no pre-lock snapshot to disagree with it, so
    // a concurrent `set-keyring` cannot make this delete the wrong entry, and
    // the existence/credential checks cannot go stale between check and write.
    //
    // `update_locked_without_validation`: dropping a credential source cannot
    // improve or worsen an unrelated invalid server, so avoid blocking on
    // whole-config validation (same rationale as remove/rename). Note the
    // resulting credential-less server is itself structurally *valid* —
    // a missing credential is an authentication-time error, not a write-time
    // one (#278). An error raised here aborts the write.
    let mut keyring_entry = (String::new(), String::new());
    let updated =
        Config::update_locked_without_validation_at(ctx.config_path_override(), |config| {
            let server = config.servers.get_mut(name).ok_or_else(|| {
                crate::error::BzrError::config(format!("server '{name}' not found"))
            })?;
            let keyring_ref = server.api_key_keyring.as_ref().ok_or_else(|| {
                crate::error::BzrError::config(format!(
                    "server '{name}' has no keyring credential to unset"
                ))
            })?;
            keyring_entry = (
                keyring_ref.service_or_default().to_string(),
                keyring_ref.account_or_default(name).to_string(),
            );
            server.api_key_keyring = None;
            Ok(())
        })?;
    // The mutator either errored (aborting the write) or set this, so an empty
    // pair here means a future edit added an early `Ok` return.
    debug_assert!(
        !keyring_entry.0.is_empty(),
        "keyring coordinates must be captured inside the mutator"
    );
    let (service_name, account_name) = keyring_entry;
    let server_url = updated
        .servers
        .get(name)
        .map(|s| s.url.clone())
        .unwrap_or_default();

    // Delete the secret only after the config write commits, so a failed write
    // never leaves the config pointing at a secret that no longer exists. The
    // reverse order loses the credential outright; this order can at worst
    // orphan an unreferenced keychain entry, which `set-keyring` overwrites.
    // (Same ordering as `config rename-server`.) Idempotent: a missing entry is
    // not an error.
    crate::credentials::keyring::delete(&service_name, &account_name)?;

    let path = Config::path_at(ctx.config_path_override())?;

    // The write above skipped validation, so the file may still be unloadable
    // because of an unrelated entry. Say so rather than reporting plain success
    // and leaving the user to hit the same error on their next command.
    if let Err(e) = updated.validate() {
        let _ = writeln!(
            w.err,
            "warning: keychain entry removed, but the config file is still not \
             loadable: {e}"
        );
    }

    let human = format!(
        "Removed keychain entry for server '{name}' (service={service_name}, \
         account={account_name}).\nThe server entry is still present but has \
         no API key source; public reads can still work. Configure \
         `--api-key-env`, `--api-key`, or `bzr config set-keyring` before \
         writes.\nConfig file: {}",
        path.display()
    );
    write_result(
        &ConfigResult::configured(name, &server_url, false, path.to_string_lossy(), true),
        &human,
        ctx.format(),
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
#[path = "keyring_tests.rs"]
mod tests;
