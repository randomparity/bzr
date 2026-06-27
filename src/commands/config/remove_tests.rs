#![expect(clippy::unwrap_used)]

//! Direct tests for the `config remove-server` leaf. Local-only command.

use crate::cli::ConfigAction;
use crate::commands::config::execute;
use crate::commands::runtime::invocation::CommandContext;
use crate::error::BzrError;
use crate::test_helpers::{
    load_config_unvalidated, run_config_action_json, seed_inline_server, setup_empty_config_env,
    CapturedIo,
};
use crate::types::output::OutputFormat;

#[tokio::test]
async fn remove_server_deletes_non_default_entry() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("keep", "https://keep.example.com", "k").await;
    seed_inline_server("drop", "https://drop.example.com", "d").await;
    // "keep" is the default (first added).

    let json = run_config_action_json(ConfigAction::RemoveServer {
        name: "drop".into(),
    })
    .await;
    assert_eq!(json["action"], "removed");
    assert_eq!(json["name"], "drop");
    assert_eq!(json["resource"], "server");

    let config = load_config_unvalidated();
    assert!(!config.servers.contains_key("drop"));
    assert!(config.servers.contains_key("keep"));
    assert_eq!(config.default_server.as_deref(), Some("keep"));
}

#[tokio::test]
async fn remove_server_missing_errors() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("only", "https://only.example.com", "x").await;

    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::RemoveServer {
            name: "ghost".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(matches!(result, Err(BzrError::Config(_))));
}

#[tokio::test]
async fn remove_server_default_with_others_refuses() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("a", "https://a.example.com", "x").await;
    seed_inline_server("b", "https://b.example.com", "y").await;
    // "a" is the default (first added); removing it while "b" remains is refused.

    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::RemoveServer { name: "a".into() },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(matches!(result, Err(BzrError::Config(_))));
    // Nothing was removed.
    assert!(load_config_unvalidated().servers.contains_key("a"));
}

#[tokio::test]
async fn remove_server_only_server_clears_default() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("solo", "https://solo.example.com", "x").await;

    let json = run_config_action_json(ConfigAction::RemoveServer {
        name: "solo".into(),
    })
    .await;
    assert_eq!(json["action"], "removed");

    let config = load_config_unvalidated();
    assert!(config.servers.is_empty());
    assert!(config.default_server.is_none());
}

#[tokio::test]
async fn remove_server_table_output_reports_human_summary() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("solo", "https://solo.example.com", "x").await;

    let mut io = CapturedIo::new();
    execute(
        &ConfigAction::RemoveServer {
            name: "solo".into(),
        },
        &CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let out = io.out_str();
    assert!(out.contains("Removed server 'solo'."));
    assert!(out.contains("Config file:"));
}

#[cfg(feature = "keyring")]
#[tokio::test]
async fn remove_server_deletes_keyring_entry() {
    use crate::test_helpers::seed_keyring_secret;

    let (_lock, _tmp) = setup_empty_config_env().await;
    crate::credentials::keyring::install_test_store();
    seed_inline_server("kr", "https://kr.example.com", "inline").await;
    seed_keyring_secret("kr", "kr-secret").await;
    // Confirm the secret is present before removal.
    assert_eq!(
        crate::credentials::keyring::retrieve("bzr", "kr").unwrap(),
        "kr-secret"
    );

    run_config_action_json(ConfigAction::RemoveServer { name: "kr".into() }).await;

    let config = load_config_unvalidated();
    assert!(!config.servers.contains_key("kr"));
    // Keychain entry is gone — retrieve now fails.
    assert!(crate::credentials::keyring::retrieve("bzr", "kr").is_err());
}

/// Regression (#300): managing one server must succeed even when an
/// unrelated server is credential-less on disk (the state `unset-keyring`
/// leaves behind). `update_locked`'s whole-config validation would reject
/// the write; remove must use the non-validating path.
#[cfg(feature = "keyring")]
#[tokio::test]
async fn remove_server_succeeds_with_other_credential_less_server() {
    use crate::test_helpers::seed_keyring_secret;

    let (_lock, _tmp) = setup_empty_config_env().await;
    crate::credentials::keyring::install_test_store();
    seed_inline_server("keepme", "https://keep.example.com", "k").await;
    seed_inline_server("dropme", "https://drop.example.com", "d").await;
    // Make "keepme" credential-less via unset-keyring after moving it to keyring.
    seed_keyring_secret("keepme", "s").await;
    execute(
        &ConfigAction::UnsetKeyring {
            name: "keepme".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::RemoveServer {
            name: "dropme".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "remove must not fail because an unrelated server is credential-less: {result:?}"
    );
    let config = load_config_unvalidated();
    assert!(!config.servers.contains_key("dropme"));
    assert!(config.servers.contains_key("keepme"));
}
