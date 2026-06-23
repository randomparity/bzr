use crate::config::Config;
use crate::error::Result;
use crate::output::result_types::{write_result, ConfigResult};
use crate::output::writers::Writers;
use crate::types::OutputFormat;

pub(super) fn set(
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

pub(super) fn unset(name: &str, format: OutputFormat, w: &mut Writers<'_>) -> Result<()> {
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
         no API key source; public reads can still work. Configure \
         `--api-key-env`, `--api-key`, or `bzr config set-keyring` before \
         writes.\nConfig file: {}",
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
