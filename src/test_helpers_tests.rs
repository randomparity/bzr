#![expect(clippy::disallowed_methods, clippy::unwrap_used)]

use super::*;

use tracing::instrument::WithSubscriber as _;

#[test]
fn xmlrpc_bug_response_contains_expected_bug_fields() {
    let xml = xmlrpc_bug_response(42, "Crash on startup");
    assert!(xml.contains("<int>42</int>"));
    assert!(xml.contains("<string>Crash on startup</string>"));
    assert!(xml.contains("<name>status</name>"));
    assert!(xml.contains("<string>NEW</string>"));
}

#[test]
fn captured_io_starts_empty() {
    let io = CapturedIo::new();
    assert!(io.out.is_empty());
    assert!(io.err.is_empty());
}

#[test]
fn captured_io_writers_route_to_owned_buffers() {
    let mut io = CapturedIo::new();
    {
        let w = io.writers();
        let _ = writeln!(w.out, "to stdout");
        let _ = writeln!(w.err, "to stderr");
    }
    assert_eq!(io.out_str(), "to stdout\n");
    assert_eq!(io.err_str(), "to stderr\n");
}

#[test]
fn tracing_capture_follows_an_async_future_across_threads() {
    let (capture, _guard) = TracingCapture::install(tracing::Level::DEBUG);
    let future = async { tracing::debug!("cross-thread tracing marker") }.with_current_subscriber();

    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(future);
    })
    .join()
    .unwrap();

    assert!(capture.output().contains("cross-thread tracing marker"));
}

fn emit_callsite_registration_marker() {
    tracing::debug!("callsite registration marker");
}

#[test]
fn tracing_capture_survives_callsite_registration_on_another_thread() {
    let (capture, _guard) = TracingCapture::install(tracing::Level::DEBUG);

    std::thread::spawn(emit_callsite_registration_marker)
        .join()
        .unwrap();
    emit_callsite_registration_marker();

    assert_eq!(
        capture
            .output()
            .matches("callsite registration marker")
            .count(),
        1
    );
}

// ── Explicit-config-path helpers (ADR-0002) ──────────────────────────
//
// These cover the `_at` companions of the ambient-root config helpers.
// They also give each companion the caller `-D dead_code` requires while
// the migrations that consume them (#820-#834) are still unlanded.

#[test]
fn setup_empty_isolated_env_yields_a_missing_config_path_under_its_own_root() {
    let (tmp, config_path) = setup_empty_isolated_env();

    assert_eq!(config_path, tmp.path().join("bzr").join("config.toml"));
    assert!(
        !config_path.exists(),
        "the empty-config helper must not write a config file"
    );
    assert!(
        !config_path.parent().unwrap().exists(),
        "the empty-config helper must leave the config directory absent, \
         exactly as an unwritten XDG root does"
    );
}

#[tokio::test]
async fn seed_inline_server_at_writes_only_to_the_given_path() {
    let (_tmp_a, path_a) = setup_empty_isolated_env();
    let (_tmp_b, path_b) = setup_empty_isolated_env();

    seed_inline_server_at(&path_a, "alpha", "https://alpha.example", "key-a").await;

    assert!(path_a.exists());
    assert!(
        !path_b.exists(),
        "seeding one explicit path must not touch another isolated root"
    );
}

#[tokio::test]
async fn load_config_at_reads_the_given_path() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "alpha", "https://alpha.example", "key-a").await;

    let config = load_config_at(&config_path);

    assert_eq!(config.servers["alpha"].url, "https://alpha.example");
    assert_eq!(config.servers["alpha"].api_key.as_deref(), Some("key-a"));
}

#[tokio::test]
async fn load_config_unvalidated_at_parses_a_config_validation_rejects() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "alpha", "https://alpha.example", "key-a").await;
    update_config_without_validation_at(&config_path, |config| {
        config.servers.get_mut("alpha").unwrap().api_key_env = Some("BZR_TEST_KEY".into());
        Ok(())
    })
    .unwrap();

    assert!(
        crate::config::Config::load_at(Some(&config_path)).is_err(),
        "two credential sources must fail validation, or this proves nothing"
    );
    let config = load_config_unvalidated_at(&config_path);
    assert_eq!(config.servers["alpha"].api_key.as_deref(), Some("key-a"));
    assert_eq!(
        config.servers["alpha"].api_key_env.as_deref(),
        Some("BZR_TEST_KEY")
    );
}

#[tokio::test]
async fn update_config_at_persists_a_validated_mutation() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "alpha", "https://alpha.example", "key-a").await;

    let updated = update_config_at(&config_path, |config| {
        config.servers.get_mut("alpha").unwrap().url = "https://beta.example".into();
        Ok(())
    })
    .unwrap();

    assert_eq!(updated.servers["alpha"].url, "https://beta.example");
    assert_eq!(
        load_config_at(&config_path).servers["alpha"].url,
        "https://beta.example"
    );
}

#[tokio::test]
async fn update_config_without_validation_at_rejects_nothing_the_validator_would() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "alpha", "https://alpha.example", "key-a").await;

    update_config_without_validation_at(&config_path, |config| {
        config.servers.get_mut("alpha").unwrap().api_key_env = Some("BZR_TEST_KEY".into());
        Ok(())
    })
    .unwrap();

    assert!(
        update_config_at(&config_path, |_| Ok(())).is_err(),
        "the validating form must reject the state the unvalidated form wrote"
    );
}

#[tokio::test]
async fn run_config_action_json_at_reports_the_given_path() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "alpha", "https://alpha.example", "key-a").await;

    let data = run_config_action_json_at(&config_path, crate::cli::ConfigAction::Show).await;

    assert_eq!(
        data.get("config_file").and_then(serde_json::Value::as_str),
        Some(config_path.to_string_lossy().as_ref())
    );
}

#[cfg(feature = "keyring")]
#[tokio::test]
async fn seed_keyring_secret_at_keys_the_shared_store_by_the_given_service() {
    crate::credentials::keyring::install_test_store();
    let (_tmp, config_path) = setup_empty_isolated_env();
    // The server name is unique to this test on purpose: the store is
    // process-global and never reset, so a name another test also seeds (the
    // `"keepme"` the helper doc names) would give the negative assertion below
    // a concurrent writer.
    let server = "seed-at-819";
    seed_inline_server_at(&config_path, server, "https://alpha.example", "key-a").await;

    seed_keyring_secret_at(&config_path, server, "secret-a", "bzr-test-819").await;

    let config = load_config_at(&config_path);
    let keyring_ref = config.servers[server].api_key_keyring.as_ref().unwrap();
    assert_eq!(keyring_ref.service.as_deref(), Some("bzr-test-819"));
    assert_eq!(
        keyring_ref.account, None,
        "the account still defaults to the server name"
    );
    assert_eq!(
        crate::credentials::keyring::retrieve("bzr-test-819", server).unwrap(),
        "secret-a"
    );
    assert!(
        crate::credentials::keyring::retrieve("bzr", server).is_err(),
        "the per-test service is what keeps the never-reset STORE map disjoint; \
         a default-'bzr' entry here would mean two tests can still collide"
    );
    // Read `BZR_KEYRING_TEST_SECRET` under the lock that serializes its
    // writers: `seed_keyring_secret_at` has already released it, and every
    // other seeding call site holds it across a whole `config::execute`.
    let _lock = crate::ENV_LOCK.lock().await;
    assert!(
        std::env::var("BZR_KEYRING_TEST_SECRET").is_err(),
        "the helper must leave no process-global residue behind"
    );
}
