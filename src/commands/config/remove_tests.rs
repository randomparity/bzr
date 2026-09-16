#![expect(clippy::unwrap_used)]

//! Direct tests for the `config remove-server` leaf. Local-only command.

use crate::cli::ConfigAction;
use crate::commands::config::execute;
use crate::commands::runtime::invocation::CommandContext;
use crate::error::BzrError;
use crate::test_helpers::{
    load_config_unvalidated_at, run_config_action_json_at, seed_inline_server_at,
    setup_empty_isolated_env, CapturedIo,
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

// One keychain `service` per keyring-touching test; see the note in
// `rename_tests.rs`. `remove_server_deletes_the_entry_named_by_the_removed_server`
// deliberately keeps the DEFAULT service for its decoy — proving the command
// reads the entry's explicit coordinates instead of falling back to defaults is
// the property under test — so the sibling that also used ("bzr", "dropme")
// moves here instead.
#[cfg(feature = "keyring")]
const REMOVE_DELETES_ENTRY_SERVICE: &str = "bzr-834-remove-deletes-entry";
#[cfg(feature = "keyring")]
const REMOVE_CRED_LESS_SERVICE: &str = "bzr-834-remove-cred-less";
#[cfg(all(unix, feature = "keyring"))]
const REMOVE_WRITE_FAILS_SERVICE: &str = "bzr-834-remove-write-fails";

#[tokio::test]
async fn remove_server_deletes_non_default_entry() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "keep", "https://keep.example.com", "k").await;
    seed_inline_server_at(&config_path, "drop", "https://drop.example.com", "d").await;
    // "keep" is the default (first added).

    let json = run_config_action_json_at(
        &config_path,
        ConfigAction::RemoveServer {
            name: "drop".into(),
        },
    )
    .await;
    assert_eq!(json["action"], "removed");
    assert_eq!(json["name"], "drop");
    assert_eq!(json["resource"], "server");

    let config = load_config_unvalidated_at(&config_path);
    assert!(!config.servers.contains_key("drop"));
    assert!(config.servers.contains_key("keep"));
    assert_eq!(config.default_server.as_deref(), Some("keep"));
}

#[tokio::test]
async fn remove_server_missing_errors() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "only", "https://only.example.com", "x").await;

    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::RemoveServer {
            name: "ghost".into(),
        },
        &ctx_at(&config_path, OutputFormat::Json),
        &mut io.writers(),
    )
    .await;
    assert!(matches!(result, Err(BzrError::Config(_))));
}

#[tokio::test]
async fn remove_server_default_with_others_refuses() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "a", "https://a.example.com", "x").await;
    seed_inline_server_at(&config_path, "b", "https://b.example.com", "y").await;
    // "a" is the default (first added); removing it while "b" remains is refused.

    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::RemoveServer { name: "a".into() },
        &ctx_at(&config_path, OutputFormat::Json),
        &mut io.writers(),
    )
    .await;
    assert!(matches!(result, Err(BzrError::Config(_))));
    // Nothing was removed.
    assert!(load_config_unvalidated_at(&config_path)
        .servers
        .contains_key("a"));
}

#[tokio::test]
async fn remove_server_only_server_clears_default() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "solo", "https://solo.example.com", "x").await;

    let json = run_config_action_json_at(
        &config_path,
        ConfigAction::RemoveServer {
            name: "solo".into(),
        },
    )
    .await;
    assert_eq!(json["action"], "removed");

    let config = load_config_unvalidated_at(&config_path);
    assert!(config.servers.is_empty());
    assert!(config.default_server.is_none());
}

#[tokio::test]
async fn remove_server_table_output_reports_human_summary() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "solo", "https://solo.example.com", "x").await;

    let mut io = CapturedIo::new();
    execute(
        &ConfigAction::RemoveServer {
            name: "solo".into(),
        },
        &ctx_at(&config_path, OutputFormat::Table),
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
    use crate::test_helpers::seed_keyring_secret_at;

    let (_tmp, config_path) = setup_empty_isolated_env();
    crate::credentials::keyring::install_test_store();
    seed_inline_server_at(&config_path, "kr", "https://kr.example.com", "inline").await;
    seed_keyring_secret_at(
        &config_path,
        "kr",
        "kr-secret",
        REMOVE_DELETES_ENTRY_SERVICE,
    )
    .await;
    // Confirm the secret is present before removal.
    assert_eq!(
        crate::credentials::keyring::retrieve(REMOVE_DELETES_ENTRY_SERVICE, "kr").unwrap(),
        "kr-secret"
    );

    run_config_action_json_at(
        &config_path,
        ConfigAction::RemoveServer { name: "kr".into() },
    )
    .await;

    let config = load_config_unvalidated_at(&config_path);
    assert!(!config.servers.contains_key("kr"));
    // Keychain entry is gone — retrieve now fails.
    assert!(crate::credentials::keyring::retrieve(REMOVE_DELETES_ENTRY_SERVICE, "kr").is_err());
}

/// Regression (#300): managing one server must succeed even when an
/// unrelated server is credential-less on disk (the state `unset-keyring`
/// leaves behind). `update_locked`'s whole-config validation would reject
/// the write; remove must use the non-validating path.
#[cfg(feature = "keyring")]
#[tokio::test]
async fn remove_server_succeeds_with_other_credential_less_server() {
    use crate::test_helpers::seed_keyring_secret_at;

    let (_tmp, config_path) = setup_empty_isolated_env();
    crate::credentials::keyring::install_test_store();
    seed_inline_server_at(&config_path, "keepme", "https://keep.example.com", "k").await;
    seed_inline_server_at(&config_path, "dropme", "https://drop.example.com", "d").await;
    // Make "keepme" credential-less via unset-keyring after moving it to keyring.
    seed_keyring_secret_at(&config_path, "keepme", "s", REMOVE_CRED_LESS_SERVICE).await;
    execute(
        &ConfigAction::UnsetKeyring {
            name: "keepme".into(),
        },
        &ctx_at(&config_path, OutputFormat::Json),
        &mut CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::RemoveServer {
            name: "dropme".into(),
        },
        &ctx_at(&config_path, OutputFormat::Json),
        &mut io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "remove must not fail because an unrelated server is credential-less: {result:?}"
    );
    let config = load_config_unvalidated_at(&config_path);
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
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "broken", "https://broken.example.com", "b").await;
    seed_inline_server_at(&config_path, "dropme", "https://drop.example.com", "d").await;

    // Hand-edited state: two credential sources on one server.
    crate::test_helpers::update_config_without_validation_at(&config_path, |config| {
        config.servers.get_mut("broken").unwrap().api_key_env = Some("BROKEN_KEY".into());
        Ok(())
    })
    .unwrap();

    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::RemoveServer {
            name: "dropme".into(),
        },
        &ctx_at(&config_path, OutputFormat::Json),
        &mut io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "remove must not be blocked by an unrelated invalid server: {result:?}"
    );

    let config = load_config_unvalidated_at(&config_path);
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
    use crate::test_helpers::seed_keyring_secret_at;
    use std::os::unix::fs::PermissionsExt as _;

    crate::credentials::keyring::install_test_store();
    let (_tmp, config_path) = setup_empty_isolated_env();

    seed_inline_server_at(&config_path, "dropme", "https://drop.example.com", "d").await;
    seed_keyring_secret_at(
        &config_path,
        "dropme",
        "drop-secret",
        REMOVE_WRITE_FAILS_SERVICE,
    )
    .await;

    // Make the config directory read-only so the locked write cannot proceed.
    let dir = config_path.parent().unwrap().to_path_buf();
    let original = std::fs::metadata(&dir).unwrap().permissions();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();

    let result = execute(
        &ConfigAction::RemoveServer {
            name: "dropme".into(),
        },
        &ctx_at(&config_path, OutputFormat::Json).with_assume_yes(true),
        &mut CapturedIo::new().writers(),
    )
    .await;

    std::fs::set_permissions(&dir, original).unwrap();
    assert!(result.is_err(), "precondition: the config write must fail");

    // The server is still configured, so its secret must still be retrievable.
    assert_eq!(
        crate::credentials::keyring::retrieve(REMOVE_WRITE_FAILS_SERVICE, "dropme").unwrap(),
        "drop-secret"
    );
    crate::credentials::keyring::delete(REMOVE_WRITE_FAILS_SERVICE, "dropme").unwrap();
}

/// The keychain coordinates come from the server entry being removed, resolved
/// from the locked read rather than the pre-lock snapshot.
///
/// A decoy secret sits at the *default* coordinates (`bzr` / server name). If
/// the command ever fell back to defaults instead of reading the entry's
/// explicit service/account, it would delete the decoy and leave the real
/// entry behind.
///
/// Note: the concurrent-modification window this guards (a `set-keyring`
/// landing between the snapshot and the lock) cannot be driven from a
/// single-threaded test without a hook in production code, so this pins the
/// resolution path rather than the race itself.
#[cfg(feature = "keyring")]
#[tokio::test]
async fn remove_server_deletes_the_entry_named_by_the_removed_server() {
    crate::credentials::keyring::install_test_store();
    // Retains ENV_LOCK: this test mutates the process-global
    // BZR_KEYRING_TEST_SECRET that `set-keyring` reads, which is the category
    // ADR-0002 keeps the lock for. It cannot delegate to `seed_keyring_secret_at`
    // (which takes the lock itself) because it needs an explicit `account`, so it
    // takes the lock directly. Config is still selected by explicit path, so no
    // XDG_CONFIG_HOME mutation is involved.
    let _lock = crate::ENV_LOCK.lock().await;
    let (_tmp, config_path) = setup_empty_isolated_env();

    seed_inline_server_at(&config_path, "dropme", "https://drop.example.com", "d").await;

    // SAFETY: Serialized via the ENV_LOCK guard held above.
    unsafe { std::env::set_var("BZR_KEYRING_TEST_SECRET", "real-secret") };
    execute(
        &ConfigAction::SetKeyring {
            name: "dropme".into(),
            service: Some("custom-svc".into()),
            account: Some("custom-acct".into()),
        },
        &ctx_at(&config_path, OutputFormat::Json),
        &mut CapturedIo::new().writers(),
    )
    .await
    .unwrap();
    // SAFETY: Serialized via the ENV_LOCK guard held above.
    unsafe { std::env::remove_var("BZR_KEYRING_TEST_SECRET") };

    crate::credentials::keyring::store("bzr", "dropme", "decoy").unwrap();

    execute(
        &ConfigAction::RemoveServer {
            name: "dropme".into(),
        },
        &ctx_at(&config_path, OutputFormat::Json).with_assume_yes(true),
        &mut CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    assert!(
        crate::credentials::keyring::retrieve("custom-svc", "custom-acct").is_err(),
        "the entry named by the removed server must be deleted"
    );
    assert_eq!(
        crate::credentials::keyring::retrieve("bzr", "dropme").unwrap(),
        "decoy",
        "the default-coordinate entry must be untouched"
    );
    crate::credentials::keyring::delete("bzr", "dropme").unwrap();
}
