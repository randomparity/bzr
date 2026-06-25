#![expect(clippy::unwrap_used)]

//! Direct tests for the `config set-keyring` / `unset-keyring` leaf.
//! Local-only command. The advisory existence and credential-source checks
//! run before any keychain access, so several error paths are testable
//! without the `keyring` feature.

use crate::cli::ConfigAction;
use crate::commands::config::execute;
use crate::commands::runtime::context::CommandContext;
use crate::error::BzrError;
use crate::test_helpers::{seed_inline_server, setup_empty_config_env, CapturedIo};
use crate::types::output::OutputFormat;

#[tokio::test]
async fn set_keyring_missing_server_errors_before_keychain() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::SetKeyring {
            name: "ghost".into(),
            service: None,
            account: None,
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(matches!(err, BzrError::Config(_)));
    assert!(err.to_string().contains("not found"));
    assert!(err.to_string().contains("set-server"));
}

#[tokio::test]
async fn unset_keyring_missing_server_errors() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::UnsetKeyring {
            name: "ghost".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(matches!(err, BzrError::Config(_)));
    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
async fn unset_keyring_without_keyring_credential_errors() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    // Inline-only server: there is no keychain credential to unset.
    seed_inline_server("inline-only", "https://inline.example.com", "secret").await;

    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::UnsetKeyring {
            name: "inline-only".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(matches!(err, BzrError::Config(_)));
    assert!(err.to_string().contains("no keyring credential"));
}

#[cfg(feature = "keyring")]
#[tokio::test]
async fn set_keyring_stores_secret_and_rewrites_config() {
    use crate::test_helpers::{load_config, seed_keyring_secret};

    crate::credentials::keyring::install_test_store();
    let (_lock, _tmp) = setup_empty_config_env().await;

    // Create an inline server first, then move it to the keychain.
    seed_inline_server("prod", "https://prod.example.com", "old-inline-value").await;
    seed_keyring_secret("prod", "new-keyring-value").await;

    // Config rewritten: inline cleared, api_key_keyring set.
    let config = load_config();
    let server = &config.servers["prod"];
    assert!(server.api_key.is_none());
    assert!(server.api_key_env.is_none());
    assert!(server.api_key_keyring.is_some());

    // Resolving the API key now fetches from the test keychain.
    assert_eq!(
        crate::credentials::resolve_api_key(server, "prod").unwrap(),
        "new-keyring-value"
    );
    crate::credentials::keyring::delete("bzr", "prod").unwrap();
}

#[cfg(feature = "keyring")]
#[tokio::test]
async fn set_keyring_table_output_reports_human_summary() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    crate::credentials::keyring::install_test_store();
    seed_inline_server("prod", "https://prod.example.com", "old-inline-value").await;

    let mut io = CapturedIo::new();
    // SAFETY: Serialized via ENV_LOCK through setup_empty_config_env.
    unsafe { std::env::set_var("BZR_KEYRING_TEST_SECRET", "new-keyring-value") };
    execute(
        &ConfigAction::SetKeyring {
            name: "prod".into(),
            service: None,
            account: None,
        },
        &CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await
    .unwrap();
    // SAFETY: Serialized via ENV_LOCK through setup_empty_config_env.
    unsafe { std::env::remove_var("BZR_KEYRING_TEST_SECRET") };

    let out = io.out_str();
    assert!(out.contains("Stored API key for server 'prod' in OS keychain"));
    assert!(out.contains("Config file:"));
    crate::credentials::keyring::delete("bzr", "prod").unwrap();
}

#[cfg(feature = "keyring")]
#[tokio::test]
async fn unset_keyring_removes_secret_and_clears_config() {
    use crate::test_helpers::{config_path, seed_keyring_secret};

    crate::credentials::keyring::install_test_store();
    let (_lock, _tmp) = setup_empty_config_env().await;

    seed_inline_server("unset-test", "https://unset-test.example.com", "tmp").await;
    seed_keyring_secret("unset-test", "unset-test-secret").await;

    execute(
        &ConfigAction::UnsetKeyring {
            name: "unset-test".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    // Reload directly: the server intentionally has no credential source now,
    // which `Config::load`'s validator would reject.
    let content = std::fs::read_to_string(config_path()).unwrap();
    let config: crate::config::Config = toml::from_str(&content).unwrap();
    let server = &config.servers["unset-test"];
    assert!(server.api_key_keyring.is_none());
    assert!(server.api_key.is_none());
    assert!(server.api_key_env.is_none());

    // Keychain entry is gone (idempotent delete returns Ok).
    crate::credentials::keyring::delete("bzr", "unset-test").unwrap();
}
