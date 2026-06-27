#![expect(clippy::unwrap_used)]

use std::path::PathBuf;

fn make_ctx(
    email: Option<&str>,
    url: &str,
    persist: bool,
    config_path: Option<PathBuf>,
) -> super::ConnectContext {
    super::ConnectContext {
        server_name: "test".into(),
        url: url.into(),
        api_key: None,
        email: email.map(str::to_owned),
        api_override: None,
        request_timeout: crate::http::REQUEST_TIMEOUT,
        retry_max: 0,
        config_path_override: config_path,
        persist,
    }
}

// ── email_hint accessor ───────────────────────────────────────────

#[test]
fn email_hint_returns_some_when_email_is_set() {
    // Kills `email_hint -> None` / `Some("")` / `Some("xyzzy")` mutants.
    let ctx = make_ctx(Some("user@example.com"), "https://example.com", false, None);
    assert_eq!(ctx.email_hint(), Some("user@example.com"));
}

#[test]
fn email_hint_returns_none_when_email_is_absent() {
    let ctx = make_ctx(None, "https://example.com", false, None);
    assert_eq!(ctx.email_hint(), None);
}

// ── hostname accessor ─────────────────────────────────────────────

#[test]
fn hostname_returns_extracted_host_not_constant() {
    // Kills `hostname -> String::new()` / `"xyzzy".into()` mutants: assert
    // different URLs yield different hostnames and match the real host part.
    let ctx_a = make_ctx(None, "https://bugzilla.example.com/", false, None);
    let ctx_b = make_ctx(None, "https://other.example.org:8080/", false, None);

    assert_eq!(ctx_a.hostname(), "bugzilla.example.com");
    assert_eq!(ctx_b.hostname(), "other.example.org");
    assert_ne!(ctx_a.hostname(), ctx_b.hostname());
    // Neither can be an empty string or the literal "xyzzy".
    assert!(!ctx_a.hostname().is_empty());
    assert_ne!(ctx_a.hostname(), "xyzzy");
}

// ── persist_locked ────────────────────────────────────────────────

#[test]
fn persist_locked_runs_mutator_when_persist_is_true() {
    // Kills `persist_locked -> Ok(())` mutant: verifies the mutator closure
    // actually executes and its change lands in the config file.
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = crate::test_helpers::write_config_to(
        &tmp,
        r#"
default_server = "test"

[servers.test]
url = "https://example.com"
api_key = "test-key"
auth_method = "header"
api_mode = "rest"
"#,
    );

    let ctx = make_ctx(None, "https://example.com", true, Some(config_path.clone()));

    // Use persist_locked to write a server_version.
    ctx.persist_locked(|config| {
        let srv = config.servers.get_mut("test").unwrap();
        srv.server_version = Some("5.2.0".into());
        Ok(())
    })
    .unwrap();

    // Reload and confirm the mutator ran.
    let reloaded = crate::config::Config::load_at(Some(&config_path)).unwrap();
    assert_eq!(
        reloaded.servers["test"].server_version.as_deref(),
        Some("5.2.0"),
        "persist_locked must execute the mutator when persist=true"
    );
}

#[test]
fn persist_locked_skips_mutator_when_persist_is_false() {
    // A no-op when persist=false (inline servers): mutator must not fire.
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = crate::test_helpers::write_config_to(
        &tmp,
        r#"
default_server = "test"

[servers.test]
url = "https://example.com"
api_key = "test-key"
auth_method = "header"
api_mode = "rest"
"#,
    );

    let ctx = make_ctx(
        None,
        "https://example.com",
        false,
        Some(config_path.clone()),
    );

    ctx.persist_locked(|config| {
        // This should NOT run.
        let srv = config.servers.get_mut("test").unwrap();
        srv.server_version = Some("SHOULD_NOT_APPEAR".into());
        Ok(())
    })
    .unwrap();

    let reloaded = crate::config::Config::load_at(Some(&config_path)).unwrap();
    assert_eq!(
        reloaded.servers["test"].server_version, None,
        "persist_locked must not run the mutator when persist=false"
    );
}

// ── resolve_connect_target email field ───────────────────────────

#[test]
fn resolve_connect_target_inline_server_carries_email() {
    // Kills the `delete field email from ServerConfig` mutant in
    // `resolve_connect_target`: asserts that an inline server's email flows
    // into the ConnectContext so it reaches email_hint().
    let inline = crate::commands::runtime::invocation::InlineServer {
        url: "https://bugzilla.example.com".into(),
        api_key_env: None,
        email: Some("user@example.com".into()),
        tls: crate::commands::runtime::invocation::InlineTlsOptions::default(),
    };
    let ctx_cmd = crate::commands::runtime::invocation::CommandContext::new(
        None,
        crate::types::OutputFormat::Json,
        None,
    )
    .with_inline_server(Some(inline));

    let target = super::resolve_connect_target(&ctx_cmd).unwrap();
    assert_eq!(
        target.ctx.email_hint(),
        Some("user@example.com"),
        "email from inline server must appear in ConnectContext.email_hint()"
    );
}

#[test]
fn resolve_connect_target_inline_server_is_ephemeral_and_uncached() {
    let inline = crate::commands::runtime::invocation::InlineServer {
        url: "https://bugzilla.example.com".into(),
        api_key_env: None,
        email: Some("user@example.com".into()),
        tls: crate::commands::runtime::invocation::InlineTlsOptions {
            pin_now: true,
            ..Default::default()
        },
    };
    let ctx_cmd = crate::commands::runtime::invocation::CommandContext::new(
        None,
        crate::types::OutputFormat::Json,
        Some(crate::types::ApiMode::Rest),
    )
    .with_config_path_override(Some(PathBuf::from("/tmp/must-not-be-read.toml")))
    .with_inline_server(Some(inline));

    let target = super::resolve_connect_target(&ctx_cmd).unwrap();

    assert!(!target.ctx.persist);
    assert!(target.ctx.config_path_override.is_none());
    assert_eq!(target.ctx.api_override, Some(crate::types::ApiMode::Rest));
    assert_eq!(target.cached_auth, None);
    assert_eq!(target.cached_mode, None);
    assert!(target.pin_current_cert);
}

#[test]
fn resolve_connect_target_config_server_is_persistent_and_uses_cached_modes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config_path = crate::test_helpers::write_config_to(
        &tmp,
        r#"
default_server = "test"

[servers.test]
url = "https://bugzilla.example.com"
api_key = "test-key"
auth_method = "header"
api_mode = "rest"
"#,
    );
    let ctx_cmd = crate::commands::runtime::invocation::CommandContext::new(
        None,
        crate::types::OutputFormat::Json,
        None,
    )
    .with_config_path_override(Some(config_path.clone()));

    let target = super::resolve_connect_target(&ctx_cmd).unwrap();

    assert!(target.ctx.persist);
    assert_eq!(
        target.ctx.config_path_override.as_deref(),
        Some(config_path.as_path())
    );
    assert_eq!(target.cached_auth, Some(crate::types::AuthMethod::Header));
    assert_eq!(target.cached_mode, Some(crate::types::ApiMode::Rest));
    assert!(!target.pin_current_cert);
}

#[test]
fn server_tls_config_copies_persisted_tls_fields() {
    let server = crate::config::ServerConfig {
        url: "https://bugzilla.example.com".into(),
        tls_ca_cert: Some(PathBuf::from("/tmp/example-ca.pem")),
        tls_pin_sha256: Some("sha256//pin-value".into()),
        tls_pin_issuer_der: Some("issuer-der".into()),
        ..crate::config::ServerConfig::default()
    };

    let tls_config = super::server_tls_config(&server, "prod");

    assert!(!tls_config.insecure);
    assert_eq!(
        tls_config.ca_cert_path.as_deref(),
        Some(std::path::Path::new("/tmp/example-ca.pem"))
    );
    assert_eq!(tls_config.pin_sha256.as_deref(), Some("sha256//pin-value"));
    assert_eq!(tls_config.pin_issuer_der.as_deref(), Some("issuer-der"));
    assert_eq!(tls_config.server_name.as_deref(), Some("prod"));
}

#[test]
fn extract_hostname_parses_url() {
    assert_eq!(
        super::extract_hostname("https://example.com/path"),
        "example.com"
    );
}

#[test]
fn extract_hostname_with_port() {
    assert_eq!(
        super::extract_hostname("https://example.com:8443/path"),
        "example.com"
    );
}

#[test]
fn extract_hostname_returns_raw_on_invalid() {
    assert_eq!(super::extract_hostname("not-a-url"), "not-a-url");
}
