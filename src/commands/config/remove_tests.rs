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

/// `remove-server` must still work when an unrelated server is *structurally
/// invalid* — the case the validation bypass exists for.
///
/// A hand-edited config can carry a server with conflicting credential sources
/// (`api_key` and `api_key_env` both set), which whole-config validation
/// rejects. Removal cannot worsen that, so it must not be blocked by it —
/// otherwise the CLI cannot repair a config it can no longer load.
///
/// The sibling `remove_server_succeeds_with_other_credential_less_server` does
/// not cover this: a credential-less server is structurally *valid*, so that
/// test passes on the validating path too.
#[tokio::test]
async fn remove_server_succeeds_with_other_structurally_invalid_server() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("broken", "https://broken.example.com", "b").await;
    seed_inline_server("dropme", "https://drop.example.com", "d").await;

    // Hand-edited state: two credential sources on one server.
    crate::test_helpers::update_config_without_validation(|config| {
        config.servers.get_mut("broken").unwrap().api_key_env = Some("BROKEN_KEY".into());
        Ok(())
    })
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
        "remove must not be blocked by an unrelated invalid server: {result:?}"
    );

    let config = load_config_unvalidated();
    assert!(!config.servers.contains_key("dropme"));
    assert!(config.servers.contains_key("broken"));
}

/// A failed config write must not destroy the keychain secret.
///
/// The delete runs only after the write commits, so a server that is still in
/// the config still has its credential. The reverse order loses the secret with
/// nothing left pointing at it.
#[cfg(all(unix, feature = "keyring"))]
#[tokio::test]
async fn remove_server_keeps_the_secret_when_the_config_write_fails() {
    use crate::test_helpers::{config_path, seed_keyring_secret};
    use std::os::unix::fs::PermissionsExt as _;

    crate::credentials::keyring::install_test_store();
    let (_lock, _tmp) = setup_empty_config_env().await;

    seed_inline_server("dropme", "https://drop.example.com", "d").await;
    seed_keyring_secret("dropme", "drop-secret").await;

    // Make the config directory read-only so the locked write cannot proceed.
    let dir = config_path().parent().unwrap().to_path_buf();
    let original = std::fs::metadata(&dir).unwrap().permissions();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();

    let result = execute(
        &ConfigAction::RemoveServer {
            name: "dropme".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None).with_assume_yes(true),
        &mut CapturedIo::new().writers(),
    )
    .await;

    std::fs::set_permissions(&dir, original).unwrap();
    assert!(result.is_err(), "precondition: the config write must fail");

    // The server is still configured, so its secret must still be retrievable.
    assert_eq!(
        crate::credentials::keyring::retrieve("bzr", "dropme").unwrap(),
        "drop-secret"
    );
    crate::credentials::keyring::delete("bzr", "dropme").unwrap();
}
