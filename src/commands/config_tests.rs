#![expect(clippy::unwrap_used)]

use super::*;
use crate::error::BzrError;
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
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    execute(
        &ConfigAction::SetServer {
            name: name.into(),
            url: url.into(),
            api_key: Some(api_key.into()),
            api_key_env: None,
            email: None,
            auth_method: None,
            tls_insecure: false,
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_now: false,
            tls_pin_clear: false,
        },
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await
    .unwrap();
}

/// Store `secret` in the test keychain for `server_name` by priming
/// the `BZR_KEYRING_TEST_SECRET` test hook and running `SetKeyring`.
#[cfg(feature = "keyring")]
async fn seed_keyring_secret(server_name: &str, secret: &str) {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
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
        &mut __cap_io.writers(),
    )
    .await
    .unwrap();
    // SAFETY: Serialized via ENV_LOCK through setup_config_env.
    unsafe { std::env::remove_var("BZR_KEYRING_TEST_SECRET") };
}

#[tokio::test]
async fn set_default_on_empty_config_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
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
        &mut __cap_io.writers(),
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
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = execute(
        &ConfigAction::SetServer {
            name: "first".into(),
            url: "https://first.example.com".into(),
            api_key: Some("first-key-1234567890".into()),
            api_key_env: None,
            email: None,
            auth_method: None,
            tls_insecure: false,
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_now: false,
            tls_pin_clear: false,
        },
        None,
        OutputFormat::Json,
        None,
        &mut __io.writers(),
    )
    .await;
    let output = __io.out_str().to_string();
    result.unwrap();
    let config = Config::load().unwrap();
    assert_eq!(config.default_server.as_deref(), Some("first"));
    assert!(config.servers.contains_key("first"));

    // The first server set must report `is_default: true` in JSON
    // output, since there was no prior default to take precedence.
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["is_default"], true);
}

#[tokio::test]
async fn second_set_server_does_not_override_default() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
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
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_now: false,
            tls_pin_clear: false,
        },
        None,
        OutputFormat::Table,
        None,
        &mut __cap_io.writers(),
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
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_now: false,
            tls_pin_clear: false,
        },
        None,
        OutputFormat::Table,
        None,
        &mut __cap_io.writers(),
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

    let mut __io2 = crate::test_helpers::CapturedIo::new();

    let result = execute(
        &ConfigAction::SetServer {
            name: "second".into(),
            url: "https://updated.example.com".into(),
            api_key: Some("updated-key-1234567890".into()),
            api_key_env: None,
            email: Some("ops@example.com".into()),
            auth_method: Some(AuthMethod::QueryParam),
            tls_insecure: true,
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_now: false,
            tls_pin_clear: false,
        },
        None,
        OutputFormat::Json,
        None,
        &mut __io2.writers(),
    )
    .await;

    let output = __io2.out_str().to_string();
    assert!(result.is_ok());

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let mut __io3 = crate::test_helpers::CapturedIo::new();

    let result = execute(
        &ConfigAction::SetDefault {
            name: "second".into(),
        },
        None,
        OutputFormat::Json,
        None,
        &mut __io3.writers(),
    )
    .await;

    let output = __io3.out_str().to_string();
    assert!(result.is_ok());

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["name"], "second");
    assert_eq!(parsed["action"], "updated");
    assert_eq!(
        Config::load().unwrap().default_server.as_deref(),
        Some("second")
    );
}

#[tokio::test]
async fn show_json_includes_populated_server_details() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
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
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_now: false,
            tls_pin_clear: false,
        },
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await
    .unwrap();

    let mut __io_a1 = crate::test_helpers::CapturedIo::new();

    let result = execute(
        &ConfigAction::Show,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a1.writers(),
    )
    .await;

    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok());

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
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
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_now: false,
            tls_pin_clear: false,
        },
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
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
    crate::credentials::keyring::install_test_store();
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

    // Resolving the API key now fetches from the test keychain.
    assert_eq!(server.resolve_api_key("prod").unwrap(), "new-keyring-value");

    // Cleanup the keychain entry so subsequent tests don't see it.
    crate::credentials::keyring::delete("bzr", "prod").unwrap();
}

#[cfg(feature = "keyring")]
#[tokio::test]
async fn migrate_to_keyring_from_inline_rewrites_config() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    crate::credentials::keyring::install_test_store();
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
        &mut __cap_io.writers(),
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
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    crate::credentials::keyring::install_test_store();
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
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_now: false,
            tls_pin_clear: false,
        },
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
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
        &mut __cap_io.writers(),
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
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    crate::credentials::keyring::install_test_store();
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
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());

    // The "different-service" entry must not exist.
    let lookup = crate::credentials::keyring::retrieve("different-service", "migrate-already-kr");
    assert!(
        lookup.is_err(),
        "no entry should have been stored at the different-service location"
    );

    // Cleanup: the original entry still exists.
    crate::credentials::keyring::delete("bzr", "migrate-already-kr").unwrap();
}

#[tokio::test]
async fn migrate_to_keyring_without_yes_errors() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
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
        &mut __cap_io.writers(),
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
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    crate::credentials::keyring::install_test_store();
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
        &mut __cap_io.writers(),
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
