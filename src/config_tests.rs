#![expect(clippy::unwrap_used, clippy::panic)]

use super::*;
use std::env;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::str::FromStr;

fn make_server_config(url: &str) -> ServerConfig {
    ServerConfig {
        url: url.to_string(),
        api_key: Some("test-key".to_string()),
        api_key_env: None,
        api_key_keyring: None,
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    }
}

fn make_config_with_server() -> Config {
    let mut servers = HashMap::new();
    servers.insert(
        "myserver".to_string(),
        make_server_config("https://bugzilla.example.com"),
    );
    Config {
        default_server: Some("myserver".to_string()),
        servers,
        templates: HashMap::new(),
        queries: HashMap::new(),
    }
}

/// Combined test for operations that require `env::set_var`.
/// Grouped in a single test to avoid env var race conditions with
/// parallel test execution.
#[test]
fn config_file_io_operations() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::tempdir().unwrap();
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    // 1. Load returns default when no file exists
    let config = Config::load().unwrap();
    assert!(config.default_server.is_none());
    assert!(config.servers.is_empty());

    // 2. Save and load roundtrip
    let original = make_config_with_server();
    original.save().unwrap();

    let loaded = Config::load().unwrap();
    assert_eq!(loaded.default_server.as_deref(), Some("myserver"));
    assert!(loaded.servers.contains_key("myserver"));
    let srv = loaded.servers.get("myserver").unwrap();
    assert_eq!(srv.url, "https://bugzilla.example.com");
    assert_eq!(srv.api_key.as_deref(), Some("test-key"));

    // 3. Re-saving an existing config preserves the file's current
    // permissions; perms are only set on first save.
    #[cfg(unix)]
    {
        let path = Config::path().unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        original.save().unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
    }

    // 4. Loading a non-NotFound IO error (e.g. the path is a
    // directory) propagates rather than being swallowed as "no
    // config".
    let _ = fs::remove_file(Config::path().unwrap());
    fs::create_dir_all(Config::path().unwrap()).unwrap();
    assert!(Config::load().is_err());
}

#[test]
fn credential_source_errors_when_no_api_key_source_defined() {
    let mut srv = make_server_config("https://example.com");
    srv.api_key = None;
    let err = srv.credential_source().unwrap_err();
    assert!(
        err.to_string().contains("must define one of"),
        "expected 'must define one of' message, got: {err}"
    );
}

#[test]
fn validate_tls_rejects_missing_ca_cert_file() {
    let mut srv = make_server_config("https://example.com");
    srv.tls_ca_cert = Some(std::path::PathBuf::from(
        "/nonexistent/path/that/should/not/exist/ca.pem",
    ));
    let err = srv.validate_tls("test").unwrap_err();
    assert!(
        err.to_string().contains("not found"),
        "expected 'not found' message, got: {err}"
    );
}

#[test]
fn validate_tls_accepts_existing_ca_cert_file() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    let mut srv = make_server_config("https://example.com");
    srv.tls_ca_cert = Some(tmp.path().to_path_buf());
    assert!(srv.validate_tls("test").is_ok());
}

#[test]
fn resolve_server_name_only_returns_default_when_none() {
    let config = make_config_with_server();
    let name = config.resolve_server_name_only(None).unwrap();
    assert_eq!(name, "myserver");
}

#[test]
fn resolve_server_name_only_returns_explicit_name() {
    let config = make_config_with_server();
    let name = config.resolve_server_name_only(Some("other")).unwrap();
    assert_eq!(name, "other");
}

#[test]
fn resolve_server_name_only_errors_when_no_default_and_no_name() {
    let config = Config::default();
    let result = config.resolve_server_name_only(None);
    assert!(result.is_err());
}

#[test]
fn resolve_server_returns_correct_server() {
    let config = make_config_with_server();
    let (name, srv) = config.resolve_server(Some("myserver")).unwrap();
    assert_eq!(name, "myserver");
    assert_eq!(srv.url, "https://bugzilla.example.com");
}

#[test]
fn resolve_server_errors_for_unknown_server() {
    let config = make_config_with_server();
    let result = config.resolve_server(Some("nonexistent"));
    assert!(result.is_err());
}

#[test]
fn api_mode_from_str_valid() {
    assert_eq!(ApiMode::from_str("rest").unwrap(), ApiMode::Rest);
    assert_eq!(ApiMode::from_str("xmlrpc").unwrap(), ApiMode::XmlRpc);
    assert_eq!(ApiMode::from_str("hybrid").unwrap(), ApiMode::Hybrid);
}

#[test]
fn api_mode_from_str_invalid() {
    let result = ApiMode::from_str("invalid");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("invalid API mode"));
}

#[test]
fn auth_method_from_str_valid() {
    assert_eq!(AuthMethod::from_str("header").unwrap(), AuthMethod::Header);
    assert_eq!(
        AuthMethod::from_str("query_param").unwrap(),
        AuthMethod::QueryParam
    );
}

#[test]
fn auth_method_from_str_invalid() {
    let result = AuthMethod::from_str("invalid");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("invalid auth method"));
}

#[test]
fn api_mode_display_formatting() {
    assert_eq!(ApiMode::Rest.to_string(), "rest");
    assert_eq!(ApiMode::XmlRpc.to_string(), "xmlrpc");
    assert_eq!(ApiMode::Hybrid.to_string(), "hybrid");
}

#[test]
fn config_roundtrips_saved_queries() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::tempdir().unwrap();
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let mut config = make_config_with_server();
    let query = crate::types::SavedQuery {
        kind: crate::types::QueryKind::List,
        product: vec!["Firefox".into()],
        status: vec!["NEW".into()],
        limit: Some(25),
        ..Default::default()
    };
    config.queries.insert("firefox-new".into(), query);
    config.save().unwrap();

    let loaded = Config::load().unwrap();
    assert!(loaded.queries.contains_key("firefox-new"));
    let q = loaded.queries.get("firefox-new").unwrap();
    assert_eq!(q.product, vec!["Firefox"]);
    assert_eq!(q.status, vec!["NEW"]);
    assert_eq!(q.limit, Some(25));
}

#[test]
fn config_empty_queries_not_serialized() {
    let config = make_config_with_server();
    let toml_str = toml::to_string_pretty(&config).unwrap();
    assert!(
        !toml_str.contains("[queries"),
        "empty queries should not appear in TOML"
    );
}

#[test]
fn config_load_rejects_multiple_api_key_sources() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::tempdir().unwrap();
    let config_dir = tmp.path().join("bzr");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        r#"
default_server = "test"

[servers.test]
url = "https://bugzilla.example.com"
api_key = "inline"
api_key_env = "BZR_TEST_API_KEY"
"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        fs::set_permissions(&config_dir, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(
            config_dir.join("config.toml"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let err = Config::load().unwrap_err();
    assert!(err.to_string().contains("server 'test'"));
    assert!(err.to_string().contains("multiple API key sources"));
}

#[test]
fn env_backed_server_resolves_api_key_from_environment() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe { env::set_var("BZR_TEST_API_KEY", "secret-from-env") };

    let server = ServerConfig {
        url: "https://bugzilla.example.com".into(),
        api_key: None,
        api_key_env: Some("BZR_TEST_API_KEY".into()),
        api_key_keyring: None,
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };

    assert_eq!(server.resolve_api_key("test").unwrap(), "secret-from-env");
    assert_eq!(
        server.credential_source_kind().unwrap(),
        CredentialSourceKind::Env
    );
}

#[test]
fn server_config_rejects_multiple_api_key_sources() {
    let server = ServerConfig {
        url: "https://bugzilla.example.com".into(),
        api_key: Some("inline".into()),
        api_key_env: Some("BZR_TEST_API_KEY".into()),
        api_key_keyring: None,
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };

    let err = server.credential_source().unwrap_err();
    assert!(err.to_string().contains("multiple API key sources"));
}

#[test]
fn keyring_ref_defaults() {
    let r = KeyringRef {
        service: None,
        account: None,
    };
    assert_eq!(r.service_or_default(), "bzr");
    assert_eq!(r.account_or_default("prod"), "prod");
}

#[test]
fn keyring_ref_explicit() {
    let r = KeyringRef {
        service: Some("custom".into()),
        account: Some("acct".into()),
    };
    assert_eq!(r.service_or_default(), "custom");
    assert_eq!(r.account_or_default("prod"), "acct");
}

#[test]
fn keyring_ref_toml_roundtrip_empty() {
    let toml_str = r#"
url = "https://example.com"
api_key_keyring = {}
"#;
    let srv: ServerConfig = toml::from_str(toml_str).unwrap();
    assert!(srv.api_key_keyring.is_some());
    let r = srv.api_key_keyring.as_ref().unwrap();
    assert!(r.service.is_none());
    assert!(r.account.is_none());
}

#[test]
fn keyring_ref_toml_roundtrip_full() {
    let toml_str = r#"
url = "https://example.com"
api_key_keyring = { service = "bzr", account = "dave" }
"#;
    let srv: ServerConfig = toml::from_str(toml_str).unwrap();
    let r = srv.api_key_keyring.as_ref().unwrap();
    assert_eq!(r.service.as_deref(), Some("bzr"));
    assert_eq!(r.account.as_deref(), Some("dave"));
}

#[test]
fn credential_source_keyring_variant() {
    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: None,
        api_key_env: None,
        api_key_keyring: Some(KeyringRef {
            service: None,
            account: None,
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };
    match server.credential_source().unwrap() {
        CredentialSource::Keyring { service, account } => {
            assert_eq!(service, "bzr");
            // account defaults handled at resolve time via server_name
            assert_eq!(account, "");
        }
        other => panic!("expected Keyring variant, got {other:?}"),
    }
    assert_eq!(
        server.credential_source_kind().unwrap(),
        CredentialSourceKind::Keyring
    );
}

#[test]
fn credential_source_rejects_keyring_with_inline() {
    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: Some("k".into()),
        api_key_env: None,
        api_key_keyring: Some(KeyringRef {
            service: None,
            account: None,
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };
    let err = server.credential_source().unwrap_err();
    assert!(err.to_string().contains("multiple API key sources"));
}

#[test]
fn credential_source_rejects_keyring_with_env() {
    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: None,
        api_key_env: Some("VAR".into()),
        api_key_keyring: Some(KeyringRef {
            service: None,
            account: None,
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };
    let err = server.credential_source().unwrap_err();
    assert!(err.to_string().contains("multiple API key sources"));
}

#[test]
fn credential_source_rejects_all_three() {
    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: Some("k".into()),
        api_key_env: Some("VAR".into()),
        api_key_keyring: Some(KeyringRef {
            service: None,
            account: None,
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };
    let err = server.credential_source().unwrap_err();
    assert!(err.to_string().contains("multiple API key sources"));
}

#[cfg(feature = "keyring")]
#[test]
fn resolve_api_key_from_keyring() {
    crate::credentials::keyring::install_test_store();
    crate::credentials::keyring::store("bzr", "resolve-test-srv1", "keyring-secret").unwrap();

    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: None,
        api_key_env: None,
        api_key_keyring: Some(KeyringRef {
            service: None,
            account: Some("resolve-test-srv1".into()),
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };

    assert_eq!(
        server.resolve_api_key("resolve-test-srv1").unwrap(),
        "keyring-secret"
    );

    // cleanup
    crate::credentials::keyring::delete("bzr", "resolve-test-srv1").unwrap();
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
        api_key: None,
        api_key_env: None,
        api_key_keyring: Some(KeyringRef {
            service: Some("resolve-test-myservice".into()),
            account: Some("resolve-test-myacct".into()),
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };

    assert_eq!(
        server.resolve_api_key("any-name").unwrap(),
        "explicit-secret"
    );

    crate::credentials::keyring::delete("resolve-test-myservice", "resolve-test-myacct").unwrap();
}

#[cfg(feature = "keyring")]
#[test]
fn resolve_api_key_from_keyring_defaults_account_to_server_name() {
    crate::credentials::keyring::install_test_store();
    // Store under the server name (no explicit account).
    crate::credentials::keyring::store("bzr", "resolve-test-srv2", "default-account-secret")
        .unwrap();

    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: None,
        api_key_env: None,
        // Neither service nor account set — should default to ("bzr", server_name).
        api_key_keyring: Some(KeyringRef {
            service: None,
            account: None,
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };

    assert_eq!(
        server.resolve_api_key("resolve-test-srv2").unwrap(),
        "default-account-secret"
    );

    crate::credentials::keyring::delete("bzr", "resolve-test-srv2").unwrap();
}

#[test]
fn validate_tls_insecure_with_ca_cert_conflicts() {
    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: Some("key".into()),
        api_key_env: None,
        api_key_keyring: None,
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: true,
        tls_ca_cert: Some(PathBuf::from("/tmp/ca.pem")),
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };
    let err = server.validate_tls("srv").unwrap_err();
    assert!(
        err.to_string().contains("mutually exclusive"),
        "error should mention 'mutually exclusive': {err}"
    );
}

#[test]
fn validate_tls_insecure_with_pin_conflicts() {
    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: Some("key".into()),
        api_key_env: None,
        api_key_keyring: None,
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: true,
        tls_ca_cert: None,
        tls_pin_sha256: Some("sha256//abc".into()),
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };
    let err = server.validate_tls("srv").unwrap_err();
    assert!(
        err.to_string().contains("mutually exclusive"),
        "error should mention 'mutually exclusive': {err}"
    );
}

#[test]
fn validate_tls_ca_cert_with_pin_conflicts() {
    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: Some("key".into()),
        api_key_env: None,
        api_key_keyring: None,
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: Some(PathBuf::from("/tmp/ca.pem")),
        tls_pin_sha256: Some("sha256//abc".into()),
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };
    let err = server.validate_tls("srv").unwrap_err();
    assert!(
        err.to_string().contains("mutually exclusive"),
        "error should mention 'mutually exclusive': {err}"
    );
}

#[test]
fn validate_tls_no_conflicts_passes() {
    let server = ServerConfig {
        url: "https://example.com".into(),
        api_key: Some("key".into()),
        api_key_env: None,
        api_key_keyring: None,
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };
    assert!(server.validate_tls("srv").is_ok());
}

#[cfg(unix)]
#[test]
fn config_save_hardens_permissions_for_new_paths() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::tempdir().unwrap();
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let config = make_config_with_server();
    config.save().unwrap();

    let config_dir = tmp.path().join("bzr");
    let config_path = config_dir.join("config.toml");
    let dir_mode = fs::metadata(config_dir).unwrap().permissions().mode() & 0o777;
    let file_mode = fs::metadata(config_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(dir_mode, 0o700);
    assert_eq!(file_mode, 0o600);
}

#[cfg(unix)]
#[test]
fn save_without_validation_hardens_recreated_file() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::tempdir().unwrap();
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    // A server left without any credential source (the unset-keyring state):
    // Config::save() would reject this, so unset-keyring uses
    // save_without_validation — which must still harden the file.
    let mut servers = HashMap::new();
    let mut srv = make_server_config("https://bugzilla.example.com");
    srv.api_key = None;
    servers.insert("myserver".to_string(), srv);
    let config = Config {
        default_server: Some("myserver".to_string()),
        servers,
        templates: HashMap::new(),
        queries: HashMap::new(),
    };

    assert!(
        config.save().is_err(),
        "credential-less config must fail validation"
    );

    let config_path = tmp.path().join("bzr").join("config.toml");
    // Force the recreation scenario: ensure no pre-hardened file exists.
    let _ = fs::remove_file(&config_path);

    config.save_without_validation().unwrap();

    let file_mode = fs::metadata(&config_path).unwrap().permissions().mode();
    assert_eq!(
        file_mode & 0o077,
        0,
        "recreated config must not be group/other accessible: {file_mode:o}"
    );
}
