//! Configuration management commands.
//!
//! Config operations are pure local file I/O — no network client or auth
//! detection needed. The function is async for signature consistency with
//! sibling command modules.

use std::fmt::Write as _;

use crate::cli::ConfigAction;
use crate::config::{Config, ServerConfig};
use crate::error::Result;
use crate::output::{self, ConfigResult};
use crate::types::OutputFormat;

#[expect(
    clippy::unused_async,
    reason = "async for signature consistency with sibling execute fns"
)]
pub async fn execute(
    action: &ConfigAction,
    _server: Option<&str>,
    format: OutputFormat,
    _api: Option<crate::types::ApiMode>,
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
        } => set_server(
            &SetServerArgs {
                name: name.as_str(),
                url: url.as_str(),
                api_key: api_key.as_deref(),
                api_key_env: api_key_env.as_deref(),
                email: email.as_deref(),
                auth_method: *auth_method,
                tls_insecure: *tls_insecure,
            },
            format,
        ),
        ConfigAction::SetDefault { name } => {
            let mut config = Config::load()?;
            if !config.servers.contains_key(name) {
                return Err(crate::error::BzrError::config(format!(
                    "server '{name}' not found"
                )));
            }
            config.default_server = Some(name.clone());
            let path = Config::path()?;
            config.save()?;

            output::print_result(
                &ConfigResult::default_set(name.as_str(), path.to_string_lossy()),
                &format!(
                    "Default server set to '{name}'\nConfig file: {}",
                    path.display()
                ),
                format,
            );
            Ok(())
        }
        ConfigAction::Show => {
            let config = Config::load()?;
            let path = Config::path()?;
            let view = output::ConfigView::from_config(&config, &path);
            output::print_config(&view, format);
            Ok(())
        }
        ConfigAction::SetKeyring {
            name,
            service,
            account,
        } => set_keyring(name, service.as_deref(), account.as_deref(), format),
        ConfigAction::UnsetKeyring { name } => unset_keyring(name.as_str(), format),
        ConfigAction::MigrateToKeyring {
            name,
            service,
            account,
            yes,
        } => migrate_to_keyring(
            name.as_str(),
            service.as_deref(),
            account.as_deref(),
            *yes,
            format,
        ),
    }
}

struct SetServerArgs<'a> {
    name: &'a str,
    url: &'a str,
    api_key: Option<&'a str>,
    api_key_env: Option<&'a str>,
    email: Option<&'a str>,
    auth_method: Option<crate::types::AuthMethod>,
    tls_insecure: bool,
}

fn set_server(args: &SetServerArgs<'_>, format: OutputFormat) -> Result<()> {
    let SetServerArgs {
        name,
        url,
        api_key,
        api_key_env,
        email,
        auth_method,
        tls_insecure,
    } = *args;
    if api_key.is_some() == api_key_env.is_some() {
        return Err(crate::error::BzrError::InputValidation(
            "provide exactly one of --api-key or --api-key-env".into(),
        ));
    }
    let mut config = Config::load()?;
    let is_update = config.servers.contains_key(name);
    config.servers.insert(
        name.to_owned(),
        ServerConfig {
            url: url.to_owned(),
            api_key: api_key.map(str::to_owned),
            api_key_env: api_key_env.map(str::to_owned),
            api_key_keyring: None,
            email: email.map(str::to_owned),
            auth_method,
            api_mode: None,
            server_version: None,
            tls_insecure,
        },
    );
    if config.default_server.is_none() {
        config.default_server = Some(name.to_owned());
    }
    let is_default = config.default_server.as_deref() == Some(name);
    let path = Config::path()?;
    config.save()?;

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

    output::print_result(
        &ConfigResult::configured(name, url, is_default, path.to_string_lossy(), is_update),
        &human,
        format,
    );
    Ok(())
}

fn set_keyring(
    name: &str,
    service: Option<&str>,
    account: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let mut config = Config::load()?;
    if !config.servers.contains_key(name) {
        return Err(crate::error::BzrError::config(format!(
            "server '{name}' not found; create it first with `bzr config set-server`"
        )));
    }

    let service_name = service.unwrap_or("bzr").to_string();
    let account_name = account.unwrap_or(name).to_string();

    let secret = read_secret_from_prompt_or_env(&service_name, &account_name)?;
    crate::credentials::keyring::store(&service_name, &account_name, &secret)?;

    let server = config
        .servers
        .get_mut(name)
        .ok_or_else(|| crate::error::BzrError::config(format!("server '{name}' disappeared")))?;
    let server_url = server.url.clone();
    server.api_key = None;
    server.api_key_env = None;
    server.api_key_keyring = Some(crate::config::KeyringRef {
        service: service.map(str::to_owned),
        account: account.map(str::to_owned),
    });
    let path = Config::path()?;
    config.save()?;

    let human = format!(
        "Stored API key for server '{name}' in OS keychain \
         (service={service_name}, account={account_name})\nConfig file: {}",
        path.display()
    );
    output::print_result(
        &ConfigResult::configured(name, &server_url, false, path.to_string_lossy(), true),
        &human,
        format,
    );
    Ok(())
}

fn unset_keyring(name: &str, format: OutputFormat) -> Result<()> {
    let mut config = Config::load()?;
    let server = config
        .servers
        .get_mut(name)
        .ok_or_else(|| crate::error::BzrError::config(format!("server '{name}' not found")))?;
    let server_url = server.url.clone();
    let keyring_ref = server.api_key_keyring.take().ok_or_else(|| {
        crate::error::BzrError::config(format!(
            "server '{name}' has no keyring credential to unset"
        ))
    })?;
    let service_name = keyring_ref.service_or_default().to_string();
    let account_name = keyring_ref.account_or_default(name).to_string();
    // Idempotent: missing entry is not an error.
    crate::credentials::keyring::delete(&service_name, &account_name)?;

    // Saving would fail validation (the server has no credential source
    // now). Write TOML directly.
    let path = Config::path()?;
    save_config_without_validation(&config, &path)?;

    let human = format!(
        "Removed keychain entry for server '{name}' (service={service_name}, \
         account={account_name}).\nThe server entry is still present but has \
         no API key source — re-run `bzr config set-server` or \
         `bzr config set-keyring` to re-credential.\nConfig file: {}",
        path.display()
    );
    output::print_result(
        &ConfigResult::configured(name, &server_url, false, path.to_string_lossy(), true),
        &human,
        format,
    );
    Ok(())
}

fn migrate_to_keyring(
    name: &str,
    service: Option<&str>,
    account: Option<&str>,
    yes: bool,
    format: OutputFormat,
) -> Result<()> {
    if !yes {
        return Err(crate::error::BzrError::InputValidation(
            "migrate-to-keyring requires --yes to confirm non-interactive migration".into(),
        ));
    }

    let mut config = Config::load()?;
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
        let server = config.servers.get_mut(name).ok_or_else(|| {
            crate::error::BzrError::config(format!("server '{name}' disappeared"))
        })?;
        server.api_key = None;
        server.api_key_keyring = Some(crate::config::KeyringRef {
            service: service.map(str::to_owned),
            account: account.map(str::to_owned),
        });
        config.save()?;
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

    output::print_result(
        &ConfigResult::configured(name, &server_url, false, path.to_string_lossy(), true),
        &human,
        format,
    );
    Ok(())
}

/// Save a config to disk without running the credential-source validator.
///
/// Used by `unset-keyring`, which intentionally leaves the server without
/// a credential source so the user can re-credential it afterward with
/// `set-server` or `set-keyring`.
fn save_config_without_validation(config: &Config, path: &std::path::Path) -> Result<()> {
    use std::fs;
    use std::io::Write as IoWrite;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(config)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(content.as_bytes())?;
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

#[cfg(not(feature = "keyring"))]
fn read_secret_from_prompt_or_env(_service: &str, _account: &str) -> crate::error::Result<String> {
    Err(crate::error::BzrError::Keyring(
        "this bzr build was compiled without keyring support; \
         rebuild with --features keyring or use api_key_env"
            .into(),
    ))
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::error::BzrError;
    use crate::test_helpers::capture_stdout;
    use crate::types::AuthMethod;

    async fn setup_config_env() -> (tokio::sync::MutexGuard<'static, ()>, tempfile::TempDir) {
        let lock = crate::ENV_LOCK.lock().await;
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join("bzr");
        std::fs::create_dir_all(&config_dir).unwrap();
        // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };
        (lock, tmp)
    }

    /// Invoke `execute` with a `SetServer` action carrying an inline
    /// `api_key` and all other optional fields at their defaults.
    async fn seed_inline_server(name: &str, url: &str, api_key: &str) {
        execute(
            &ConfigAction::SetServer {
                name: name.into(),
                url: url.into(),
                api_key: Some(api_key.into()),
                api_key_env: None,
                email: None,
                auth_method: None,
                tls_insecure: false,
            },
            None,
            OutputFormat::Json,
            None,
        )
        .await
        .unwrap();
    }

    /// Store `secret` in the (mock) keychain for `server_name` by priming
    /// the `BZR_KEYRING_TEST_SECRET` test hook and running `SetKeyring`.
    #[cfg(feature = "keyring")]
    async fn seed_keyring_secret(server_name: &str, secret: &str) {
        // SAFETY: Serialized via ENV_LOCK through setup_config_env.
        unsafe { std::env::set_var("BZR_KEYRING_TEST_SECRET", secret) };
        execute(
            &ConfigAction::SetKeyring {
                name: server_name.into(),
                service: None,
                account: None,
            },
            None,
            OutputFormat::Json,
            None,
        )
        .await
        .unwrap();
        // SAFETY: Serialized via ENV_LOCK through setup_config_env.
        unsafe { std::env::remove_var("BZR_KEYRING_TEST_SECRET") };
    }

    #[tokio::test]
    async fn set_default_on_empty_config_returns_error() {
        let (_lock, _tmp) = setup_config_env().await;
        let config = Config::default();
        config.save().unwrap();
        let result = execute(
            &ConfigAction::SetDefault {
                name: "nonexistent".into(),
            },
            None,
            OutputFormat::Table,
            None,
        )
        .await;
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), BzrError::Config(_)),
            "expected Config error for unknown server"
        );
    }

    #[tokio::test]
    async fn first_set_server_auto_sets_default() {
        let (_lock, _tmp) = setup_config_env().await;
        execute(
            &ConfigAction::SetServer {
                name: "first".into(),
                url: "https://first.example.com".into(),
                api_key: Some("first-key-1234567890".into()),
                api_key_env: None,
                email: None,
                auth_method: None,
                tls_insecure: false,
            },
            None,
            OutputFormat::Table,
            None,
        )
        .await
        .unwrap();
        let config = Config::load().unwrap();
        assert_eq!(config.default_server.as_deref(), Some("first"));
        assert!(config.servers.contains_key("first"));
    }

    #[tokio::test]
    async fn second_set_server_does_not_override_default() {
        let (_lock, _tmp) = setup_config_env().await;
        // Set up first server
        execute(
            &ConfigAction::SetServer {
                name: "first".into(),
                url: "https://first.example.com".into(),
                api_key: Some("first-key-1234567890".into()),
                api_key_env: None,
                email: None,
                auth_method: None,
                tls_insecure: false,
            },
            None,
            OutputFormat::Table,
            None,
        )
        .await
        .unwrap();
        // Add second server
        execute(
            &ConfigAction::SetServer {
                name: "second".into(),
                url: "https://second.example.com".into(),
                api_key: Some("second-key-1234567890".into()),
                api_key_env: None,
                email: None,
                auth_method: None,
                tls_insecure: false,
            },
            None,
            OutputFormat::Table,
            None,
        )
        .await
        .unwrap();
        let config = Config::load().unwrap();
        assert_eq!(
            config.default_server.as_deref(),
            Some("first"),
            "second server should not override existing default"
        );
        assert_eq!(config.servers.len(), 2);
    }

    #[tokio::test]
    async fn set_server_update_preserves_existing_default() {
        let (_lock, _tmp) = setup_config_env().await;
        for (name, url) in [
            ("first", "https://first.example.com"),
            ("second", "https://second.example.com"),
        ] {
            seed_inline_server(name, url, &format!("{name}-key-1234567890")).await;
        }

        let (result, output) = capture_stdout(execute(
            &ConfigAction::SetServer {
                name: "second".into(),
                url: "https://updated.example.com".into(),
                api_key: Some("updated-key-1234567890".into()),
                api_key_env: None,
                email: Some("ops@example.com".into()),
                auth_method: Some(AuthMethod::QueryParam),
                tls_insecure: true,
            },
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_ok());

        let parsed = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["name"], "second");
        assert_eq!(parsed["action"], "updated");

        let config = Config::load().unwrap();
        assert_eq!(config.default_server.as_deref(), Some("first"));
        let server = &config.servers["second"];
        assert_eq!(server.url, "https://updated.example.com");
        assert_eq!(server.email.as_deref(), Some("ops@example.com"));
        assert_eq!(server.auth_method, Some(AuthMethod::QueryParam));
        assert!(server.tls_insecure);
    }

    #[tokio::test]
    async fn set_default_persists_selected_server() {
        let (_lock, _tmp) = setup_config_env().await;
        for (name, url) in [
            ("first", "https://first.example.com"),
            ("second", "https://second.example.com"),
        ] {
            seed_inline_server(name, url, &format!("{name}-key-1234567890")).await;
        }

        let (result, output) = capture_stdout(execute(
            &ConfigAction::SetDefault {
                name: "second".into(),
            },
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_ok());

        let parsed = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["name"], "second");
        assert_eq!(parsed["action"], "updated");
        assert_eq!(
            Config::load().unwrap().default_server.as_deref(),
            Some("second")
        );
    }

    #[tokio::test]
    async fn show_json_includes_populated_server_details() {
        let (_lock, _tmp) = setup_config_env().await;
        execute(
            &ConfigAction::SetServer {
                name: "prod".into(),
                url: "https://prod.example.com".into(),
                api_key: Some("abcdef1234567890".into()),
                api_key_env: None,
                email: Some("admin@example.com".into()),
                auth_method: Some(AuthMethod::Header),
                tls_insecure: true,
            },
            None,
            OutputFormat::Json,
            None,
        )
        .await
        .unwrap();

        let (result, output) =
            capture_stdout(execute(&ConfigAction::Show, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());

        let parsed = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["default_server"], "prod");
        assert_eq!(parsed["servers"]["prod"]["url"], "https://prod.example.com");
        assert_eq!(parsed["servers"]["prod"]["email"], "admin@example.com");
        assert_eq!(parsed["servers"]["prod"]["auth_method"], "header");
        assert_eq!(parsed["servers"]["prod"]["tls_insecure"], true);
        assert_eq!(parsed["servers"]["prod"]["api_key"], "abcdef12...");
        assert_eq!(parsed["servers"]["prod"]["api_key_source"], "inline");
    }

    #[tokio::test]
    async fn set_server_with_env_var_persists_env_source() {
        let (_lock, _tmp) = setup_config_env().await;
        execute(
            &ConfigAction::SetServer {
                name: "prod".into(),
                url: "https://prod.example.com".into(),
                api_key: None,
                api_key_env: Some("BZR_API_KEY".into()),
                email: None,
                auth_method: None,
                tls_insecure: false,
            },
            None,
            OutputFormat::Json,
            None,
        )
        .await
        .unwrap();

        let config = Config::load().unwrap();
        let server = &config.servers["prod"];
        assert_eq!(server.api_key, None);
        assert_eq!(server.api_key_env.as_deref(), Some("BZR_API_KEY"));
    }

    #[cfg(feature = "keyring")]
    #[tokio::test]
    async fn set_keyring_stores_secret_and_rewrites_config() {
        ::keyring::set_default_credential_builder(::keyring::mock::default_credential_builder());
        let (_lock, _tmp) = setup_config_env().await;

        // Create an inline server first.
        seed_inline_server("prod", "https://prod.example.com", "old-inline-value").await;

        // Inject the test secret via the BZR_KEYRING_TEST_SECRET env var so we
        // don't need an interactive stdin.
        seed_keyring_secret("prod", "new-keyring-value").await;

        // Config should have been rewritten: inline cleared, api_key_keyring set.
        let config = Config::load().unwrap();
        let server = &config.servers["prod"];
        assert!(server.api_key.is_none());
        assert!(server.api_key_env.is_none());
        assert!(server.api_key_keyring.is_some());

        // Resolving the API key now fetches from the (mock) keychain.
        assert_eq!(server.resolve_api_key("prod").unwrap(), "new-keyring-value");

        // Cleanup the keychain entry so subsequent tests don't see it.
        crate::credentials::keyring::delete("bzr", "prod").unwrap();
    }

    #[cfg(feature = "keyring")]
    #[tokio::test]
    async fn migrate_to_keyring_from_inline_rewrites_config() {
        ::keyring::set_default_credential_builder(::keyring::mock::default_credential_builder());
        let (_lock, _tmp) = setup_config_env().await;

        seed_inline_server(
            "migrate-inline",
            "https://migrate-inline.example.com",
            "inline-secret-value",
        )
        .await;

        execute(
            &ConfigAction::MigrateToKeyring {
                name: "migrate-inline".into(),
                service: None,
                account: None,
                yes: true,
            },
            None,
            OutputFormat::Json,
            None,
        )
        .await
        .unwrap();

        let config = Config::load().unwrap();
        let server = &config.servers["migrate-inline"];
        assert!(server.api_key.is_none(), "inline key should be cleared");
        assert!(server.api_key_keyring.is_some());
        assert_eq!(
            server.resolve_api_key("migrate-inline").unwrap(),
            "inline-secret-value"
        );
        crate::credentials::keyring::delete("bzr", "migrate-inline").unwrap();
    }

    #[cfg(feature = "keyring")]
    #[tokio::test]
    async fn migrate_to_keyring_from_env_preserves_config() {
        ::keyring::set_default_credential_builder(::keyring::mock::default_credential_builder());
        let (_lock, _tmp) = setup_config_env().await;

        // SAFETY: Serialized via ENV_LOCK through setup_config_env.
        unsafe { std::env::set_var("BZR_MIGRATE_TEST_KEY", "env-secret-value") };
        execute(
            &ConfigAction::SetServer {
                name: "migrate-env".into(),
                url: "https://migrate-env.example.com".into(),
                api_key: None,
                api_key_env: Some("BZR_MIGRATE_TEST_KEY".into()),
                email: None,
                auth_method: None,
                tls_insecure: false,
            },
            None,
            OutputFormat::Json,
            None,
        )
        .await
        .unwrap();

        execute(
            &ConfigAction::MigrateToKeyring {
                name: "migrate-env".into(),
                service: None,
                account: None,
                yes: true,
            },
            None,
            OutputFormat::Json,
            None,
        )
        .await
        .unwrap();
        unsafe { std::env::remove_var("BZR_MIGRATE_TEST_KEY") };

        let config = Config::load().unwrap();
        let server = &config.servers["migrate-env"];
        // Env source preserved — config.toml is NOT rewritten.
        assert_eq!(server.api_key_env.as_deref(), Some("BZR_MIGRATE_TEST_KEY"));
        assert!(server.api_key_keyring.is_none());

        // The secret IS in the keychain.
        let stored = crate::credentials::keyring::retrieve("bzr", "migrate-env").unwrap();
        assert_eq!(stored, "env-secret-value");
        crate::credentials::keyring::delete("bzr", "migrate-env").unwrap();
    }

    #[cfg(feature = "keyring")]
    #[tokio::test]
    async fn migrate_to_keyring_from_keyring_errors_before_storing() {
        ::keyring::set_default_credential_builder(::keyring::mock::default_credential_builder());
        let (_lock, _tmp) = setup_config_env().await;

        // Set up a keyring-backed server.
        seed_inline_server("migrate-already-kr", "https://example.com", "init").await;
        seed_keyring_secret("migrate-already-kr", "original-secret").await;

        // Attempt to migrate with a DIFFERENT service. Must error, and
        // must NOT have written anything to the new service.
        let result = execute(
            &ConfigAction::MigrateToKeyring {
                name: "migrate-already-kr".into(),
                service: Some("different-service".into()),
                account: None,
                yes: true,
            },
            None,
            OutputFormat::Json,
            None,
        )
        .await;
        assert!(result.is_err());

        // The "different-service" entry must not exist.
        let lookup =
            crate::credentials::keyring::retrieve("different-service", "migrate-already-kr");
        assert!(
            lookup.is_err(),
            "no entry should have been stored at the different-service location"
        );

        // Cleanup: the original entry still exists.
        crate::credentials::keyring::delete("bzr", "migrate-already-kr").unwrap();
    }

    #[tokio::test]
    async fn migrate_to_keyring_without_yes_errors() {
        let (_lock, _tmp) = setup_config_env().await;

        seed_inline_server(
            "migrate-noyes",
            "https://migrate-noyes.example.com",
            "secret",
        )
        .await;

        let result = execute(
            &ConfigAction::MigrateToKeyring {
                name: "migrate-noyes".into(),
                service: None,
                account: None,
                yes: false,
            },
            None,
            OutputFormat::Json,
            None,
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, crate::error::BzrError::InputValidation(_)));
        assert!(err.to_string().contains("--yes"));
    }

    #[cfg(feature = "keyring")]
    #[tokio::test]
    async fn unset_keyring_removes_secret_and_clears_config() {
        ::keyring::set_default_credential_builder(::keyring::mock::default_credential_builder());
        let (_lock, _tmp) = setup_config_env().await;

        // Build a keyring-backed server by first creating an inline one,
        // then running set-keyring to migrate to the keychain.
        seed_inline_server("unset-test", "https://unset-test.example.com", "tmp").await;
        seed_keyring_secret("unset-test", "unset-test-secret").await;

        // Now unset.
        execute(
            &ConfigAction::UnsetKeyring {
                name: "unset-test".into(),
            },
            None,
            OutputFormat::Json,
            None,
        )
        .await
        .unwrap();

        // Reload config directly (bypass Config::load's validator — the
        // server intentionally has no credential source).
        let path = Config::path().unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let config: Config = toml::from_str(&content).unwrap();
        let server = &config.servers["unset-test"];
        assert!(server.api_key_keyring.is_none());
        assert!(server.api_key.is_none());
        assert!(server.api_key_env.is_none());

        // Keychain entry is gone (idempotent delete returns Ok).
        crate::credentials::keyring::delete("bzr", "unset-test").unwrap();
    }
}
