#![expect(clippy::unwrap_used)]

//! Direct tests for the `config rename-server` leaf. Local-only command.

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
async fn rename_server_preserves_inline_credentials() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("stage", "https://stage.example.com", "secret-key").await;

    let json = run_config_action_json(ConfigAction::RenameServer {
        old: "stage".into(),
        new: "staging".into(),
    })
    .await;
    assert_eq!(json["action"], "renamed");
    assert_eq!(json["name"], "staging");
    assert_eq!(json["previous_name"], "stage");

    let config = load_config_unvalidated();
    assert!(!config.servers.contains_key("stage"));
    let server = &config.servers["staging"];
    assert_eq!(server.url, "https://stage.example.com");
    assert_eq!(server.api_key.as_deref(), Some("secret-key"));
    // Renamed server was the only one, so it stays the default.
    assert_eq!(config.default_server.as_deref(), Some("staging"));
}

#[tokio::test]
async fn rename_server_updates_default_pointer() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("a", "https://a.example.com", "x").await;
    seed_inline_server("b", "https://b.example.com", "y").await;
    // "a" is default; rename it and confirm the pointer follows.
    run_config_action_json(ConfigAction::RenameServer {
        old: "a".into(),
        new: "a2".into(),
    })
    .await;
    let config = load_config_unvalidated();
    assert_eq!(config.default_server.as_deref(), Some("a2"));
    assert!(config.servers.contains_key("a2"));
    assert!(config.servers.contains_key("b"));
}

#[tokio::test]
async fn rename_server_old_missing_errors() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("real", "https://real.example.com", "x").await;
    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::RenameServer {
            old: "ghost".into(),
            new: "new".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(matches!(result, Err(BzrError::Config(_))));
}

#[tokio::test]
async fn rename_server_new_exists_errors() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("a", "https://a.example.com", "x").await;
    seed_inline_server("b", "https://b.example.com", "y").await;
    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::RenameServer {
            old: "a".into(),
            new: "b".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(matches!(result, Err(BzrError::Config(_))));
    assert!(
        result.unwrap_err().to_string().contains("already exists"),
        "renaming onto an existing alias must report the collision"
    );
}

#[tokio::test]
async fn rename_server_same_name_errors() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("same", "https://same.example.com", "x").await;
    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::RenameServer {
            old: "same".into(),
            new: "same".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    let err = result.unwrap_err();
    assert!(matches!(err, BzrError::Config(_)));
    assert!(err.to_string().contains("must differ"));
    // The server is untouched.
    assert!(load_config_unvalidated().servers.contains_key("same"));
}

#[tokio::test]
async fn rename_server_table_output_reports_human_summary() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("stage", "https://stage.example.com", "x").await;

    let mut io = CapturedIo::new();
    execute(
        &ConfigAction::RenameServer {
            old: "stage".into(),
            new: "staging".into(),
        },
        &CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let out = io.out_str();
    assert!(out.contains("Renamed server 'stage' to 'staging'."));
    assert!(out.contains("Config file:"));
}

#[cfg(feature = "keyring")]
#[tokio::test]
async fn rename_server_moves_keyring_secret() {
    use crate::test_helpers::seed_keyring_secret;

    let (_lock, _tmp) = setup_empty_config_env().await;
    crate::credentials::keyring::install_test_store();
    seed_inline_server("oldname", "https://kr.example.com", "inline").await;
    seed_keyring_secret("oldname", "kr-secret").await;

    run_config_action_json(ConfigAction::RenameServer {
        old: "oldname".into(),
        new: "newname".into(),
    })
    .await;

    let config = load_config_unvalidated();
    assert!(config.servers.contains_key("newname"));
    assert!(config.servers["newname"].api_key_keyring.is_some());
    // Secret reachable under the new default account, gone under the old.
    assert_eq!(
        crate::credentials::keyring::retrieve("bzr", "newname").unwrap(),
        "kr-secret"
    );
    assert!(crate::credentials::keyring::retrieve("bzr", "oldname").is_err());
}

/// Kill `kr.account.is_none() → true` mutant (rename.rs:39): when the
/// keyring entry uses an **explicit** account the secret belongs to that
/// account, not the server-name default, so the rename must NOT move it.
/// With the mutant (guard always true) the move fires unconditionally,
/// attempting to retrieve from the default account and failing / corrupting.
#[cfg(feature = "keyring")]
#[tokio::test]
async fn rename_server_does_not_move_explicitly_accounted_keyring_secret() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    crate::credentials::keyring::install_test_store();

    // Seed a server with an inline key first, then manually swap it for a
    // keyring entry with an explicit account so the rename guard condition
    // is `account.is_some()` (guard should be false → no move).
    seed_inline_server("svc", "https://svc.example.com", "dummy").await;

    // Replace the inline key with a keyring ref that has an explicit account.
    crate::test_helpers::update_config_without_validation(|config| {
        let srv = config.servers.get_mut("svc").unwrap();
        srv.api_key = None;
        srv.api_key_keyring = Some(crate::config::KeyringRef {
            service: Some("bzr".into()),
            account: Some("explicit-acct".into()),
        });
        Ok(())
    })
    .unwrap();

    // Store the real secret under the explicit account so we can verify
    // it is not disturbed.
    crate::credentials::keyring::store("bzr", "explicit-acct", "explicit-secret").unwrap();

    // Rename — must succeed and must NOT touch the keyring entry.
    let json = run_config_action_json(ConfigAction::RenameServer {
        old: "svc".into(),
        new: "svc2".into(),
    })
    .await;
    assert_eq!(json["action"], "renamed");

    // The secret must still be reachable under the original explicit account.
    assert_eq!(
        crate::credentials::keyring::retrieve("bzr", "explicit-acct").unwrap(),
        "explicit-secret",
        "rename must not move a keyring entry that uses an explicit account"
    );
    // And must NOT have been mirrored to the new server name.
    assert!(
        crate::credentials::keyring::retrieve("bzr", "svc2").is_err(),
        "rename must not create a server-name entry when account is explicit"
    );
}

/// Regression (#300): rename must also succeed when an unrelated server is
/// credential-less on disk (same `read_unvalidated` + non-validating write
/// path as remove).
#[cfg(feature = "keyring")]
#[tokio::test]
async fn rename_server_succeeds_with_other_credential_less_server() {
    use crate::test_helpers::seed_keyring_secret;

    let (_lock, _tmp) = setup_empty_config_env().await;
    crate::credentials::keyring::install_test_store();
    seed_inline_server("keepme", "https://keep.example.com", "k").await;
    seed_inline_server("rename-me", "https://r.example.com", "r").await;
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
        &ConfigAction::RenameServer {
            old: "rename-me".into(),
            new: "renamed".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "rename must not fail because an unrelated server is credential-less: {result:?}"
    );
    let config = load_config_unvalidated();
    assert!(config.servers.contains_key("renamed"));
    assert!(!config.servers.contains_key("rename-me"));
}
