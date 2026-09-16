#![expect(clippy::unwrap_used)]

//! Direct tests for the `config migrate-to-keyring` leaf. Local-only command.

use crate::cli::ConfigAction;
use crate::commands::config::execute;
use crate::commands::runtime::invocation::CommandContext;
use crate::config::ServerConfig;
use crate::error::BzrError;
use crate::test_helpers::{
    seed_inline_server_at, setup_empty_isolated_env, update_config_without_validation_at,
    CapturedIo,
};
use crate::types::output::OutputFormat;

use std::path::Path;

/// A command context pinned to an explicit config path, so config resolution
/// never consults `XDG_CONFIG_HOME` and the test needs no `ENV_LOCK`
/// (ADR-0002).
fn ctx_at(config_path: &Path, format: OutputFormat) -> CommandContext {
    CommandContext::new(None, format, None)
        .with_config_path_override(Some(config_path.to_path_buf()))
}

// The other migrate tests keep the default `"bzr"` service: their server names
// ("migrate-inline", "mig", "migrate-env") are unique across the suite, so the
// `(service, account)` keys of the shared test store stay disjoint without help.
// This one seeds through `seed_keyring_secret_at`, which requires an explicit
// service.
#[cfg(feature = "keyring")]
const MIGRATE_ALREADY_KEYRING_SERVICE: &str = "bzr-834-migrate-already-keyring";

/// Build a `MigrateToKeyring` action with default service/account and the
/// given confirmation flag.
fn migrate_action(name: &str, yes: bool) -> ConfigAction {
    ConfigAction::MigrateToKeyring {
        name: name.into(),
        service: None,
        account: None,
        yes,
    }
}

#[tokio::test]
async fn migrate_to_keyring_without_yes_errors() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "noyes", "https://noyes.example.com", "secret").await;

    let mut io = CapturedIo::new();
    let result = execute(
        &migrate_action("noyes", false),
        &ctx_at(&config_path, OutputFormat::Json),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(matches!(err, BzrError::InputValidation { .. }));
    assert!(err.to_string().contains("--yes"));
}

#[tokio::test]
async fn migrate_to_keyring_missing_server_errors() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "real", "https://real.example.com", "secret").await;

    let mut io = CapturedIo::new();
    let result = execute(
        &migrate_action("ghost", true),
        &ctx_at(&config_path, OutputFormat::Json),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(matches!(err, BzrError::Config(_)));
    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
async fn migrate_to_keyring_without_api_key_source_errors() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    update_config_without_validation_at(&config_path, |config| {
        config.servers.insert(
            "public".into(),
            ServerConfig {
                url: "https://public.example.com".into(),
                ..ServerConfig::default()
            },
        );
        config.default_server = Some("public".into());
        Ok(())
    })
    .unwrap();

    let mut io = CapturedIo::new();
    let result = execute(
        &migrate_action("public", true),
        &ctx_at(&config_path, OutputFormat::Json),
        &mut io.writers(),
    )
    .await;

    assert!(matches!(
        result,
        Err(BzrError::Config(ref msg)) if msg.contains("no API key source")
    ));
}

#[tokio::test]
async fn migrate_to_keyring_rejects_login_tokens() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    update_config_without_validation_at(&config_path, |config| {
        config.servers.insert(
            "token".into(),
            ServerConfig {
                url: "https://token.example.com".into(),
                token: Some("login-token".into()),
                ..ServerConfig::default()
            },
        );
        config.default_server = Some("token".into());
        Ok(())
    })
    .unwrap();

    let result = execute(
        &migrate_action("token", true),
        &ctx_at(&config_path, OutputFormat::Json),
        &mut CapturedIo::new().writers(),
    )
    .await;

    assert!(matches!(
        result,
        Err(BzrError::Config(ref message)) if message.contains("token keyring storage")
    ));
}

#[cfg(feature = "keyring")]
#[tokio::test]
async fn migrate_to_keyring_from_inline_rewrites_config() {
    use crate::test_helpers::load_config_at;

    let (_tmp, config_path) = setup_empty_isolated_env();
    crate::credentials::keyring::install_test_store();
    seed_inline_server_at(
        &config_path,
        "migrate-inline",
        "https://migrate-inline.example.com",
        "inline-secret-value",
    )
    .await;

    execute(
        &migrate_action("migrate-inline", true),
        &ctx_at(&config_path, OutputFormat::Json),
        &mut CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    let config = load_config_at(&config_path);
    let server = &config.servers["migrate-inline"];
    assert!(server.api_key.is_none(), "inline key should be cleared");
    assert!(server.api_key_keyring.is_some());
    assert_eq!(
        crate::credentials::resolve_api_key(server, "migrate-inline").unwrap(),
        "inline-secret-value"
    );
    crate::credentials::keyring::delete("bzr", "migrate-inline").unwrap();
}

#[cfg(feature = "keyring")]
#[tokio::test]
async fn migrate_to_keyring_inline_table_output_reports_human_summary() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    crate::credentials::keyring::install_test_store();
    seed_inline_server_at(
        &config_path,
        "mig",
        "https://mig.example.com",
        "inline-secret-value",
    )
    .await;

    let mut io = CapturedIo::new();
    execute(
        &migrate_action("mig", true),
        &ctx_at(&config_path, OutputFormat::Table),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let out = io.out_str();
    assert!(out.contains("Migrated server 'mig' from inline API key to OS keychain"));
    assert!(out.contains("Config file:"));
    crate::credentials::keyring::delete("bzr", "mig").unwrap();
}

#[cfg(feature = "keyring")]
#[tokio::test]
async fn migrate_to_keyring_from_env_preserves_config() {
    use crate::test_helpers::load_config_at;

    // Retains ENV_LOCK: this test mutates the process-global
    // BZR_MIGRATE_TEST_KEY that `api_key_env` resolution reads, which is the
    // category ADR-0002 keeps the lock for. Config is still selected by
    // explicit path, so no XDG_CONFIG_HOME mutation is involved.
    let _lock = crate::ENV_LOCK.lock().await;
    let (_tmp, config_path) = setup_empty_isolated_env();
    crate::credentials::keyring::install_test_store();

    // SAFETY: Serialized via the ENV_LOCK guard held above.
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
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_now: false,
            tls_pin_clear: false,
        },
        &ctx_at(&config_path, OutputFormat::Json),
        &mut CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    execute(
        &migrate_action("migrate-env", true),
        &ctx_at(&config_path, OutputFormat::Json),
        &mut CapturedIo::new().writers(),
    )
    .await
    .unwrap();
    // SAFETY: Serialized via the ENV_LOCK guard held above.
    unsafe { std::env::remove_var("BZR_MIGRATE_TEST_KEY") };

    let config = load_config_at(&config_path);
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
    use crate::test_helpers::seed_keyring_secret_at;

    let (_tmp, config_path) = setup_empty_isolated_env();
    crate::credentials::keyring::install_test_store();

    // Set up a keyring-backed server.
    seed_inline_server_at(
        &config_path,
        "migrate-already-kr",
        "https://example.com",
        "init",
    )
    .await;
    seed_keyring_secret_at(
        &config_path,
        "migrate-already-kr",
        "original-secret",
        MIGRATE_ALREADY_KEYRING_SERVICE,
    )
    .await;

    // Attempt to migrate with a DIFFERENT service. Must error, and must NOT
    // have written anything to the new service.
    let result = execute(
        &ConfigAction::MigrateToKeyring {
            name: "migrate-already-kr".into(),
            service: Some("different-service".into()),
            account: None,
            yes: true,
        },
        &ctx_at(&config_path, OutputFormat::Json),
        &mut CapturedIo::new().writers(),
    )
    .await;
    assert!(result.is_err());

    // The "different-service" entry must not exist.
    let lookup = crate::credentials::keyring::retrieve("different-service", "migrate-already-kr");
    assert!(
        lookup.is_err(),
        "no entry should have been stored at the different-service location"
    );
    crate::credentials::keyring::delete(MIGRATE_ALREADY_KEYRING_SERVICE, "migrate-already-kr")
        .unwrap();
}
