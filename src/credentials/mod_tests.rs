#![expect(clippy::unwrap_used)]

use crate::config::{KeyringRef, ServerConfig};

fn server_with_env(var_name: &str) -> ServerConfig {
    ServerConfig {
        url: "https://example.com".into(),
        api_key_env: Some(var_name.into()),
        ..ServerConfig::default()
    }
}

#[test]
fn resolve_optional_api_key_returns_none_without_source() {
    let server = ServerConfig {
        url: "https://example.com".into(),
        ..ServerConfig::default()
    };

    assert_eq!(
        super::resolve_optional_api_key(&server, "public").unwrap(),
        None
    );
}

#[test]
fn resolve_api_key_from_environment() {
    let _redaction_guard = crate::bugzilla_auth::active_api_key_test_guard(None);
    let _lock = crate::ENV_LOCK.blocking_lock();
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe { std::env::set_var("BZR_CREDENTIALS_TEST_KEY", "secret-from-env") };

    let server = server_with_env("BZR_CREDENTIALS_TEST_KEY");

    assert_eq!(
        super::resolve_api_key(&server, "test").unwrap(),
        "secret-from-env"
    );
    assert_eq!(
        crate::bugzilla_auth::redact_api_key("rejected secret-from-env"),
        "rejected [REDACTED]"
    );
}

#[test]
fn resolve_api_key_reports_missing_environment_variable() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe { std::env::remove_var("BZR_CREDENTIALS_TEST_MISSING") };

    let server = server_with_env("BZR_CREDENTIALS_TEST_MISSING");
    let err = super::resolve_api_key(&server, "test").unwrap_err();

    assert!(err.to_string().contains("BZR_CREDENTIALS_TEST_MISSING"));
}

#[cfg(feature = "keyring")]
#[test]
fn resolve_api_key_from_keyring_with_explicit_service_and_account() {
    crate::credentials::keyring::install_test_store();
    crate::credentials::keyring::store(
        "resolve-test-myservice",
        "resolve-test-myacct",
        "explicit-secret",
    )
    .unwrap();

    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key_keyring: Some(KeyringRef {
            service: Some("resolve-test-myservice".into()),
            account: Some("resolve-test-myacct".into()),
        }),
        ..ServerConfig::default()
    };

    assert_eq!(
        super::resolve_api_key(&server, "any-name").unwrap(),
        "explicit-secret"
    );

    crate::credentials::keyring::delete("resolve-test-myservice", "resolve-test-myacct").unwrap();
}

#[cfg(feature = "keyring")]
#[test]
fn resolve_api_key_from_keyring_defaults_account_to_server_name() {
    crate::credentials::keyring::install_test_store();
    crate::credentials::keyring::store("bzr", "resolve-test-srv2", "default-account-secret")
        .unwrap();

    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key_keyring: Some(KeyringRef {
            service: None,
            account: None,
        }),
        ..ServerConfig::default()
    };

    assert_eq!(
        super::resolve_api_key(&server, "resolve-test-srv2").unwrap(),
        "default-account-secret"
    );

    crate::credentials::keyring::delete("bzr", "resolve-test-srv2").unwrap();
}
