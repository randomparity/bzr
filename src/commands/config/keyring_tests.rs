#![expect(clippy::unwrap_used)]

//! Direct tests for the `config set-keyring` / `unset-keyring` leaf.
//! Local-only command. The advisory existence and credential-source checks
//! run before any keychain access, so several error paths are testable
//! without the `keyring` feature.

use crate::cli::ConfigAction;
use crate::commands::config::execute;
use crate::commands::runtime::invocation::CommandContext;
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
    use crate::test_helpers::{load_config, seed_keyring_secret};

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

    // The validating loader accepts the credential-less server `unset-keyring`
    // leaves behind: missing credentials are an authentication-time error, not
    // a structural one.
    let config = load_config();
    let server = &config.servers["unset-test"];
    assert!(server.api_key_keyring.is_none());
    assert!(server.api_key.is_none());
    assert!(server.api_key_env.is_none());

    // Keychain entry is gone (idempotent delete returns Ok).
    crate::credentials::keyring::delete("bzr", "unset-test").unwrap();
}

// ── Regression: config stays editable after `unset-keyring` (issue #278) ──
//
// `unset-keyring` leaves a server with zero credential sources. These tests pin
// the rule that such a server is structurally valid, so the config commands that
// call `Config::load_at` / `Config::update_locked_at` keep working instead of
// wedging until the entry is hand-removed.

/// `set-keyring` must re-credential a server that `unset-keyring` just emptied.
#[cfg(feature = "keyring")]
#[tokio::test]
async fn unset_keyring_then_set_keyring_round_trips() {
    use crate::test_helpers::{load_config, seed_keyring_secret};

    crate::credentials::keyring::install_test_store();
    let (_lock, _tmp) = setup_empty_config_env().await;

    seed_inline_server("roundtrip", "https://roundtrip.example.com", "init").await;
    seed_keyring_secret("roundtrip", "first-secret").await;

    execute(
        &ConfigAction::UnsetKeyring {
            name: "roundtrip".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    // `seed_keyring_secret` runs `set-keyring` and panics on error, so reaching
    // the assertions below is itself the "did not wedge" check.
    seed_keyring_secret("roundtrip", "second-secret").await;

    let config = load_config();
    let server = &config.servers["roundtrip"];
    assert!(
        server.api_key_keyring.is_some(),
        "keyring ref should be restored"
    );
    assert_eq!(
        crate::credentials::resolve_api_key(server, "roundtrip").unwrap(),
        "second-secret"
    );

    crate::credentials::keyring::delete("bzr", "roundtrip").unwrap();
}

/// `set-default` must succeed while an *unrelated* server is credential-less.
/// It goes through `update_locked_at`, which validates the whole post-mutation
/// config — so a credential-less peer must not abort the write.
#[cfg(feature = "keyring")]
#[tokio::test]
async fn unset_keyring_then_set_default_succeeds() {
    use crate::test_helpers::seed_keyring_secret;

    crate::credentials::keyring::install_test_store();
    let (_lock, _tmp) = setup_empty_config_env().await;

    seed_inline_server("alpha", "https://alpha.example.com", "alpha-key").await;
    seed_inline_server("beta", "https://beta.example.com", "beta-tmp").await;
    seed_keyring_secret("beta", "beta-secret").await;

    execute(
        &ConfigAction::UnsetKeyring {
            name: "beta".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    let result = execute(
        &ConfigAction::SetDefault {
            name: "alpha".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut CapturedIo::new().writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "set-default must succeed while 'beta' is credential-less: {:?}",
        result.unwrap_err()
    );
    // Assert the effect, not just the exit status: a silently no-op
    // `set-default` would also return Ok.
    assert_eq!(
        crate::test_helpers::load_config().default_server.as_deref(),
        Some("alpha")
    );

    crate::credentials::keyring::delete("bzr", "beta").unwrap();
}

/// `set-keyring` deliberately does *not* share `unset-keyring`'s bypass: adding
/// a credential is an administrative operation that can introduce new structural
/// errors, so it fails fast on an already-broken config.
///
/// This pins that boundary. Without it, a future "make these consistent" refactor
/// could extend the bypass to every config command and break no test.
#[cfg(feature = "keyring")]
#[tokio::test]
async fn set_keyring_is_blocked_by_other_structurally_invalid_server() {
    crate::credentials::keyring::install_test_store();
    let (_lock, _tmp) = setup_empty_config_env().await;

    seed_inline_server("broken", "https://broken.example.com", "b").await;
    seed_inline_server("target", "https://target.example.com", "t").await;

    crate::test_helpers::update_config_without_validation(|config| {
        config.servers.get_mut("broken").unwrap().api_key_env = Some("BROKEN_KEY".into());
        Ok(())
    })
    .unwrap();

    // SAFETY: Serialized via ENV_LOCK held by setup_empty_config_env.
    unsafe { std::env::set_var("BZR_KEYRING_TEST_SECRET", "s") };
    let result = execute(
        &ConfigAction::SetKeyring {
            name: "target".into(),
            service: None,
            account: None,
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut CapturedIo::new().writers(),
    )
    .await;
    // SAFETY: Serialized via ENV_LOCK held by setup_empty_config_env.
    unsafe { std::env::remove_var("BZR_KEYRING_TEST_SECRET") };

    let err = result.unwrap_err();
    assert!(matches!(err, BzrError::Config(_)));
    assert!(
        err.to_string().contains("broken"),
        "the error should name the offending server, got: {err}"
    );
}

/// `unset-keyring` when the *target itself* is structurally invalid (two
/// credential sources). The unvalidated read newly allows this, and the result
/// is a repair: only `api_key_keyring` is cleared, the other source survives.
#[cfg(feature = "keyring")]
#[tokio::test]
async fn unset_keyring_repairs_a_target_with_conflicting_sources() {
    use crate::test_helpers::{load_config_unvalidated, seed_keyring_secret};

    crate::credentials::keyring::install_test_store();
    let (_lock, _tmp) = setup_empty_config_env().await;

    seed_inline_server("dual", "https://dual.example.com", "d").await;
    seed_keyring_secret("dual", "dual-secret").await;

    // set-keyring clears the other sources; put one back so the target itself
    // carries two credential sources.
    crate::test_helpers::update_config_without_validation(|config| {
        config.servers.get_mut("dual").unwrap().api_key_env = Some("DUAL_KEY".into());
        Ok(())
    })
    .unwrap();

    let result = execute(
        &ConfigAction::UnsetKeyring {
            name: "dual".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut CapturedIo::new().writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "unset-keyring must work on a target with conflicting sources: {result:?}"
    );

    // Net effect is repair: the conflict is gone and the config loads again.
    let server = &load_config_unvalidated().servers["dual"];
    assert!(server.api_key_keyring.is_none());
    assert_eq!(server.api_key_env.as_deref(), Some("DUAL_KEY"));
    crate::test_helpers::load_config();
}

/// `unset-keyring` must still work when an unrelated server is *structurally
/// invalid* — the case its validation bypass exists for. See
/// `remove_server_succeeds_with_other_structurally_invalid_server`.
#[cfg(feature = "keyring")]
#[tokio::test]
async fn unset_keyring_succeeds_with_other_structurally_invalid_server() {
    use crate::test_helpers::{load_config_unvalidated, seed_keyring_secret};

    crate::credentials::keyring::install_test_store();
    let (_lock, _tmp) = setup_empty_config_env().await;

    seed_inline_server("broken", "https://broken.example.com", "b").await;
    seed_inline_server("target", "https://target.example.com", "t").await;
    seed_keyring_secret("target", "target-secret").await;

    // Hand-edited state: two credential sources on one server.
    crate::test_helpers::update_config_without_validation(|config| {
        config.servers.get_mut("broken").unwrap().api_key_env = Some("BROKEN_KEY".into());
        Ok(())
    })
    .unwrap();

    let result = execute(
        &ConfigAction::UnsetKeyring {
            name: "target".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut CapturedIo::new().writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "unset-keyring must not be blocked by an unrelated invalid server: {result:?}"
    );

    let config = load_config_unvalidated();
    assert!(config.servers["target"].api_key_keyring.is_none());
    assert!(config.servers.contains_key("broken"));
}

/// `set-server` must re-credential a server that `unset-keyring` just emptied.
#[cfg(feature = "keyring")]
#[tokio::test]
async fn unset_keyring_then_set_server_recredentials_successfully() {
    use crate::test_helpers::{load_config, seed_keyring_secret};

    crate::credentials::keyring::install_test_store();
    let (_lock, _tmp) = setup_empty_config_env().await;

    seed_inline_server("recred", "https://recred.example.com", "init").await;
    seed_keyring_secret("recred", "kr-secret").await;

    execute(
        &ConfigAction::UnsetKeyring {
            name: "recred".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    let result = execute(
        &ConfigAction::SetServer {
            name: "recred".into(),
            url: "https://recred.example.com".into(),
            api_key: None,
            api_key_env: Some("RECRED_API_KEY".into()),
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
    .await;
    assert!(
        result.is_ok(),
        "set-server must succeed after unset-keyring: {:?}",
        result.unwrap_err()
    );

    let config = load_config();
    assert_eq!(
        config.servers["recred"].api_key_env.as_deref(),
        Some("RECRED_API_KEY")
    );
}

/// A failed config write must not destroy the keychain secret.
///
/// The delete runs only after the write commits, so a config that still points
/// at the keyring entry still has a secret behind it. The reverse order leaves
/// the server referencing a credential that no longer exists, which fails later
/// with a confusing "entry not found" instead of a clear missing-credential
/// error.
#[cfg(all(unix, feature = "keyring"))]
#[tokio::test]
async fn unset_keyring_keeps_the_secret_when_the_config_write_fails() {
    use crate::test_helpers::{config_path, load_config_unvalidated, seed_keyring_secret};
    use std::os::unix::fs::PermissionsExt as _;

    crate::credentials::keyring::install_test_store();
    let (_lock, _tmp) = setup_empty_config_env().await;

    seed_inline_server("wfail", "https://wfail.example.com", "w").await;
    seed_keyring_secret("wfail", "wfail-secret").await;

    // Make the config directory read-only so the locked write cannot proceed.
    let dir = config_path().parent().unwrap().to_path_buf();
    let original = std::fs::metadata(&dir).unwrap().permissions();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();

    let result = execute(
        &ConfigAction::UnsetKeyring {
            name: "wfail".into(),
        },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut CapturedIo::new().writers(),
    )
    .await;

    std::fs::set_permissions(&dir, original).unwrap();
    assert!(result.is_err(), "precondition: the config write must fail");

    // The config still references the keyring entry, so the secret must remain.
    assert!(load_config_unvalidated().servers["wfail"]
        .api_key_keyring
        .is_some());
    assert_eq!(
        crate::credentials::keyring::retrieve("bzr", "wfail").unwrap(),
        "wfail-secret"
    );
    crate::credentials::keyring::delete("bzr", "wfail").unwrap();
}
