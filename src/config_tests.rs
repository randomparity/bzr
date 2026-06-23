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

    // 3. Re-saving an existing config always produces a 0o600 file because
    // atomic_write creates the temp with 0o600 and rename replaces the
    // destination inode entirely (the old inode's permissions are not kept).
    #[cfg(unix)]
    {
        let path = Config::path().unwrap();
        original.save().unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    // 4. Loading a non-NotFound IO error (e.g. the path is a
    // directory) propagates rather than being swallowed as "no
    // config".
    let _ = fs::remove_file(Config::path().unwrap());
    fs::create_dir_all(Config::path().unwrap()).unwrap();
    assert!(Config::load().is_err());
}

#[test]
fn credential_source_allows_no_api_key_source() {
    let mut srv = make_server_config("https://example.com");
    srv.api_key = None;

    assert!(srv.credential_source().unwrap().is_none());
    assert!(srv.validate("public").is_ok());
}

#[test]
fn resolve_optional_api_key_none_without_source() {
    let mut srv = make_server_config("https://example.com");
    srv.api_key = None;

    assert_eq!(srv.resolve_optional_api_key("public").unwrap(), None);
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
        Some(CredentialSourceKind::Env)
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
        Some(CredentialSource::Keyring { service, account }) => {
            assert_eq!(service, "bzr");
            // account defaults handled at resolve time via server_name
            assert_eq!(account, "");
        }
        other => panic!("expected Keyring variant, got {other:?}"),
    }
    assert_eq!(
        server.credential_source_kind().unwrap(),
        Some(CredentialSourceKind::Keyring)
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

    // A server without any credential source must still be saved with hardened
    // file permissions.
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

#[test]
fn failed_write_leaves_previous_config_intact() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::TempDir::new().unwrap();
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    // Seed v1.
    let mut v1 = Config::default();
    v1.servers
        .insert("v1".to_string(), make_server_config("https://v1.test"));
    v1.save().unwrap();
    let before = std::fs::read(Config::path().unwrap()).unwrap();

    // Arm the post-temp fault seam and attempt to write v2; it must fail.
    set_fail_after_temp(true);
    let mut v2 = Config::default();
    v2.servers
        .insert("v2".to_string(), make_server_config("https://v2.test"));
    let result = v2.save();
    set_fail_after_temp(false);
    assert!(result.is_err(), "armed write must fail");

    // The on-disk config must be byte-identical to v1, and no temp remains.
    let after = std::fs::read(Config::path().unwrap()).unwrap();
    assert_eq!(
        before, after,
        "failed write must leave the old config intact"
    );
    let dir = tmp.path().join("bzr");
    let temps: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            std::path::Path::new(n)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("tmp"))
        })
        .collect();
    assert!(
        temps.is_empty(),
        "failed write must clean up its temp: {temps:?}"
    );
}

#[test]
fn save_leaves_no_temp_files_and_writes_complete_content() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::TempDir::new().unwrap();
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let mut config = Config::default();
    config
        .servers
        .insert("a".to_string(), make_server_config("https://a.test"));
    config.save().unwrap();

    let dir = tmp.path().join("bzr");
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            std::path::Path::new(n)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("tmp"))
        })
        .collect();
    assert!(
        leftovers.is_empty(),
        "no temp files should remain: {leftovers:?}"
    );

    let reloaded = Config::load().unwrap();
    assert!(
        reloaded.servers.contains_key("a"),
        "content must be complete"
    );
}

#[test]
fn overwrite_replaces_content_wholesale() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::TempDir::new().unwrap();
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let mut config = Config::default();
    config
        .servers
        .insert("v1".to_string(), make_server_config("https://v1.test"));
    config.save().unwrap();

    let mut config2 = Config::default();
    config2
        .servers
        .insert("v2".to_string(), make_server_config("https://v2.test"));
    config2.save().unwrap();

    let reloaded = Config::load().unwrap();
    assert!(reloaded.servers.contains_key("v2"));
    assert!(!reloaded.servers.contains_key("v1"));
}

#[cfg(unix)]
#[test]
fn saved_config_file_is_0600() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::TempDir::new().unwrap();
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let mut config = Config::default();
    config
        .servers
        .insert("a".to_string(), make_server_config("https://a.test"));
    config.save().unwrap();

    let path = Config::path().unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "config file must be owner-only");
}

#[test]
fn save_reaps_old_crash_orphaned_temp_files() {
    use std::time::{Duration, SystemTime};

    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::TempDir::new().unwrap();
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let dir = tmp.path().join("bzr");
    std::fs::create_dir_all(&dir).unwrap();
    // An orphan left by a crash long ago: backdate its mtime past the gate.
    let orphan = dir.join("config.toml.99999.7.tmp");
    std::fs::write(&orphan, "stale").unwrap();
    let old = SystemTime::now() - Duration::from_secs(7200);
    std::fs::File::options()
        .write(true)
        .open(&orphan)
        .unwrap()
        .set_modified(old)
        .unwrap();

    let mut config = Config::default();
    config
        .servers
        .insert("a".to_string(), make_server_config("https://a.test"));
    config.save().unwrap();

    assert!(!orphan.exists(), "an hour-old orphan temp must be reaped");
}

#[test]
fn save_preserves_fresh_temp_files_of_concurrent_writers() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::TempDir::new().unwrap();
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let dir = tmp.path().join("bzr");
    std::fs::create_dir_all(&dir).unwrap();
    // A *fresh* sibling temp, as another bzr process would have mid-write.
    let live = dir.join("config.toml.12345.0.tmp");
    std::fs::write(&live, "in-flight").unwrap();

    let mut config = Config::default();
    config
        .servers
        .insert("a".to_string(), make_server_config("https://a.test"));
    config.save().unwrap();

    assert!(
        live.exists(),
        "a fresh concurrent-writer temp must NOT be reaped (would lose its write)"
    );
}

#[cfg(unix)]
#[test]
fn fsync_parent_dir_errors_when_parent_cannot_be_opened() {
    let tmp = tempfile::TempDir::new().unwrap();
    let parent = tmp.path().join("missing-parent");
    let path = parent.join("config.toml");

    let err = super::fsync_parent_dir(&path).unwrap_err().to_string();
    let parent = parent.to_string_lossy();

    assert!(
        err.contains("open config parent directory") && err.contains(parent.as_ref()),
        "error should name parent directory and failed operation, got: {err}"
    );
}

#[test]
fn update_locked_preserves_disjoint_concurrent_edits() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::TempDir::new().unwrap();
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    // Seed a server with neither pin nor auth_method set.
    let mut base = Config::default();
    base.servers
        .insert("s".to_string(), make_server_config("https://s.test"));
    base.save().unwrap();

    // "Process A": set the TLS pin under the lock.
    Config::update_locked(|cfg| {
        let srv = cfg.servers.get_mut("s").unwrap();
        srv.tls_pin_sha256 =
            Some("sha256//AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".to_string());
        Ok(())
    })
    .unwrap();

    // "Process B": set a DISJOINT field (auth_method) under the lock. Because
    // update_locked reloads from disk first, it sees A's pin and must not drop it.
    Config::update_locked(|cfg| {
        let srv = cfg.servers.get_mut("s").unwrap();
        srv.auth_method = Some(crate::types::AuthMethod::Header);
        Ok(())
    })
    .unwrap();

    let reloaded = Config::load().unwrap();
    let srv = reloaded.servers.get("s").unwrap();
    assert_eq!(
        srv.tls_pin_sha256.as_deref(),
        Some("sha256//AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="),
        "A's pin must survive B's disjoint edit (reload-under-lock)"
    );
    assert_eq!(
        srv.auth_method,
        Some(crate::types::AuthMethod::Header),
        "B's auth_method must be applied"
    );
}

#[test]
fn update_locked_rejects_reentrant_call() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::TempDir::new().unwrap();
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let result = Config::update_locked(|_outer| {
        // A nested write from inside a closure must be rejected, not deadlock.
        Config::update_locked(|_inner| Ok(()))?;
        Ok(())
    });

    assert!(result.is_err(), "nested update_locked must error");
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("re-entrantly"),
        "expected a re-entrancy error, got: {err}"
    );
}

#[test]
fn update_locked_can_create_a_server_not_yet_persisted() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::TempDir::new().unwrap();
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    // No prior save — disk has no config at all.
    Config::update_locked(|config| {
        config.servers.insert(
            "fresh".to_string(),
            make_server_config("https://fresh.test"),
        );
        Ok(())
    })
    .unwrap();

    let reloaded = Config::load().unwrap();
    assert!(
        reloaded.servers.contains_key("fresh"),
        "an upserting closure must create a server from an empty on-disk config"
    );
}

#[test]
fn update_locked_reloads_a_credential_less_config_and_can_heal_it() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    let tmp = tempfile::TempDir::new().unwrap();
    // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
    unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    // Seed a server, then strip its only credential source WITHOUT validation
    // (mirrors `bzr config unset-keyring`).
    let mut base = Config::default();
    base.servers
        .insert("s".to_string(), make_server_config("https://s.test"));
    base.save().unwrap();
    Config::update_locked_without_validation(|cfg| {
        let srv = cfg.servers.get_mut("s").unwrap();
        srv.api_key = None;
        srv.api_key_env = None;
        srv.api_key_keyring = None;
        Ok(())
    })
    .unwrap();

    // A validating `update_locked` that re-credentials the server must SUCCEED:
    // the reload is unvalidated, the mutation restores a credential, and the
    // post-mutation validation then passes.
    Config::update_locked(|cfg| {
        let srv = cfg.servers.get_mut("s").unwrap();
        srv.api_key_env = Some("BZR_KEY".to_string());
        Ok(())
    })
    .unwrap();

    let reloaded = Config::load().unwrap();
    assert_eq!(
        reloaded.servers.get("s").unwrap().api_key_env.as_deref(),
        Some("BZR_KEY")
    );
}

/// `Config::path()` precedence (#304): `--config` override > `BZR_CONFIG`
/// env > `$XDG_CONFIG_HOME/bzr/config.toml`. Combined into one test under
/// `ENV_LOCK` to avoid env races, and cleans up the override + env so later
/// tests see default resolution.
#[test]
fn config_path_resolution_precedence() {
    let _lock = crate::ENV_LOCK.blocking_lock();
    Config::set_path_override(None);
    // SAFETY: serialized via ENV_LOCK; no other thread reads these concurrently.
    unsafe {
        env::remove_var("BZR_CONFIG");
        env::set_var("XDG_CONFIG_HOME", "/tmp/bzr-xdg-test-home");
    }

    // Default: XDG dir + bzr/config.toml.
    assert_eq!(
        Config::path().unwrap(),
        PathBuf::from("/tmp/bzr-xdg-test-home/bzr/config.toml")
    );

    // BZR_CONFIG (full file path) overrides the default dir.
    unsafe { env::set_var("BZR_CONFIG", "/tmp/bzr-env-config.toml") };
    assert_eq!(
        Config::path().unwrap(),
        PathBuf::from("/tmp/bzr-env-config.toml")
    );

    // The --config override beats BZR_CONFIG.
    Config::set_path_override(Some(PathBuf::from("/tmp/bzr-flag-config.toml")));
    assert_eq!(
        Config::path().unwrap(),
        PathBuf::from("/tmp/bzr-flag-config.toml")
    );

    // Clearing the override falls back to BZR_CONFIG.
    Config::set_path_override(None);
    assert_eq!(
        Config::path().unwrap(),
        PathBuf::from("/tmp/bzr-env-config.toml")
    );

    // An empty BZR_CONFIG is ignored (falls through to the default dir).
    unsafe { env::set_var("BZR_CONFIG", "") };
    assert_eq!(
        Config::path().unwrap(),
        PathBuf::from("/tmp/bzr-xdg-test-home/bzr/config.toml")
    );

    // Cleanup: restore default resolution for subsequent tests.
    unsafe { env::remove_var("BZR_CONFIG") };
    Config::set_path_override(None);
}
