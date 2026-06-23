#![expect(clippy::unwrap_used)]

//! Direct tests for the `config migrate-to-keyring` leaf. Local-only command.

use crate::cli::ConfigAction;
use crate::commands::config::execute;
use crate::commands::runtime::context::CommandContext;
use crate::config::ServerConfig;
use crate::error::BzrError;
use crate::test_helpers::{
    seed_inline_server, setup_empty_config_env, update_config_without_validation, CapturedIo,
};
use crate::types::common::OutputFormat;

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
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("noyes", "https://noyes.example.com", "secret").await;

    let mut io = CapturedIo::new();
    let result = execute(
        &migrate_action("noyes", false),
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(matches!(err, BzrError::InputValidation(_)));
    assert!(err.to_string().contains("--yes"));
}

#[tokio::test]
async fn migrate_to_keyring_missing_server_errors() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("real", "https://real.example.com", "secret").await;

    let mut io = CapturedIo::new();
    let result = execute(
        &migrate_action("ghost", true),
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(matches!(err, BzrError::Config(_)));
    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
async fn migrate_to_keyring_without_api_key_source_errors() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    update_config_without_validation(|config| {
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
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(matches!(
        result,
        Err(BzrError::Config(ref msg)) if msg.contains("no API key source")
    ));
}

#[cfg(feature = "keyring")]
#[tokio::test]
async fn migrate_to_keyring_from_inline_rewrites_config() {
    use crate::test_helpers::load_config;

    let (_lock, _tmp) = setup_empty_config_env().await;
    crate::credentials::keyring::install_test_store();
    seed_inline_server(
        "migrate-inline",
        "https://migrate-inline.example.com",
        "inline-secret-value",
    )
    .await;

    execute(
        &migrate_action("migrate-inline", true),
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    let config = load_config();
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
async fn migrate_to_keyring_inline_table_output_reports_human_summary() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    crate::credentials::keyring::install_test_store();
    seed_inline_server("mig", "https://mig.example.com", "inline-secret-value").await;

    let mut io = CapturedIo::new();
    execute(
        &migrate_action("mig", true),
        &CommandContext::new(None, OutputFormat::Table, None),
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
    use crate::test_helpers::load_config;

    let (_lock, _tmp) = setup_empty_config_env().await;
    crate::credentials::keyring::install_test_store();

    // SAFETY: Serialized via ENV_LOCK through setup_empty_config_env.
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
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    execute(
        &migrate_action("migrate-env", true),
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut CapturedIo::new().writers(),
    )
    .await
    .unwrap();
    // SAFETY: Serialized via ENV_LOCK through setup_empty_config_env.
    unsafe { std::env::remove_var("BZR_MIGRATE_TEST_KEY") };

    let config = load_config();
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
    use crate::test_helpers::seed_keyring_secret;

    let (_lock, _tmp) = setup_empty_config_env().await;
    crate::credentials::keyring::install_test_store();

    // Set up a keyring-backed server.
    seed_inline_server("migrate-already-kr", "https://example.com", "init").await;
    seed_keyring_secret("migrate-already-kr", "original-secret").await;

    // Attempt to migrate with a DIFFERENT service. Must error, and must NOT
    // have written anything to the new service.
    let result = execute(
        &ConfigAction::MigrateToKeyring {
            name: "migrate-already-kr".into(),
            service: Some("different-service".into()),
            account: None,
            yes: true,
        },
        &CommandContext::new(None, OutputFormat::Json, None),
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
    crate::credentials::keyring::delete("bzr", "migrate-already-kr").unwrap();
}
