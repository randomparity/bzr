#![expect(clippy::unwrap_used)]

//! Direct tests for the `config set-server` leaf. Local-only command: no
//! network client is constructed, so these never start a mock server.

use crate::cli::ConfigAction;
use crate::commands::config::execute;
use crate::commands::runtime::context::CommandContext;
use crate::config::ServerConfig;
use crate::test_helpers::{
    config_path, load_config, seed_inline_server, setup_empty_config_env,
    update_config_without_validation, CapturedIo,
};
use crate::types::output::OutputFormat;
use crate::types::transport::AuthMethod;

/// Owned, all-defaulted view of the `SetServer` operands so each test can
/// override only the fields it cares about via `..Default::default()` (the
/// enum variant itself does not support record-update syntax).
#[derive(Default)]
struct Sv {
    name: String,
    url: String,
    api_key: Option<String>,
    api_key_env: Option<String>,
    email: Option<String>,
    auth_method: Option<AuthMethod>,
    tls_insecure: bool,
    tls_pin_sha256: Option<String>,
    tls_pin_now: bool,
    tls_pin_clear: bool,
}

impl Sv {
    fn new(name: &str, url: &str) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            ..Self::default()
        }
    }

    fn into_action(self) -> ConfigAction {
        ConfigAction::SetServer {
            name: self.name,
            url: self.url,
            api_key: self.api_key,
            api_key_env: self.api_key_env,
            email: self.email,
            auth_method: self.auth_method,
            tls_insecure: self.tls_insecure,
            tls_ca_cert: None,
            tls_pin_sha256: self.tls_pin_sha256,
            tls_pin_now: self.tls_pin_now,
            tls_pin_clear: self.tls_pin_clear,
        }
    }
}

#[tokio::test]
async fn first_set_server_auto_sets_default() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    let mut io = CapturedIo::new();
    let action = Sv {
        api_key: Some("first-key-1234567890".into()),
        ..Sv::new("first", "https://first.example.com")
    }
    .into_action();
    execute(
        &action,
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let config = load_config();
    assert_eq!(config.default_server.as_deref(), Some("first"));
    assert!(config.servers.contains_key("first"));

    // The first server set must report `is_default: true` and `created`,
    // since there was no prior default to take precedence.
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["is_default"], true);
    assert_eq!(parsed["action"], "created");
}

#[tokio::test]
async fn second_set_server_does_not_override_default() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("first", "https://first.example.com", "first-key-1234567890").await;
    seed_inline_server(
        "second",
        "https://second.example.com",
        "second-key-1234567890",
    )
    .await;

    let config = load_config();
    assert_eq!(
        config.default_server.as_deref(),
        Some("first"),
        "second server should not override existing default"
    );
    assert_eq!(config.servers.len(), 2);
}

#[tokio::test]
async fn set_server_update_preserves_existing_default() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    for (name, url) in [
        ("first", "https://first.example.com"),
        ("second", "https://second.example.com"),
    ] {
        seed_inline_server(name, url, &format!("{name}-key-1234567890")).await;
    }

    let mut io = CapturedIo::new();
    let action = Sv {
        api_key: Some("updated-key-1234567890".into()),
        email: Some("ops@example.com".into()),
        auth_method: Some(AuthMethod::QueryParam),
        tls_insecure: true,
        ..Sv::new("second", "https://updated.example.com")
    }
    .into_action();
    execute(
        &action,
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["name"], "second");
    assert_eq!(parsed["action"], "updated");

    let config = load_config();
    assert_eq!(config.default_server.as_deref(), Some("first"));
    let server = &config.servers["second"];
    assert_eq!(server.url, "https://updated.example.com");
    assert_eq!(server.email.as_deref(), Some("ops@example.com"));
    assert_eq!(server.auth_method, Some(AuthMethod::QueryParam));
    assert!(server.tls_insecure);
}

#[tokio::test]
async fn set_server_with_env_var_persists_env_source() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    let mut io = CapturedIo::new();
    let action = Sv {
        api_key_env: Some("BZR_API_KEY".into()),
        ..Sv::new("prod", "https://prod.example.com")
    }
    .into_action();
    execute(
        &action,
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let config = load_config();
    let server = &config.servers["prod"];
    assert_eq!(server.api_key, None);
    assert_eq!(server.api_key_env.as_deref(), Some("BZR_API_KEY"));
}

#[tokio::test]
async fn set_server_allows_url_without_api_key_source() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    let mut io = CapturedIo::new();
    let result = execute(
        &Sv::new("public", "https://public.example.com").into_action(),
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "set-server failed: {result:?}");
    let config = load_config();
    let server = &config.servers["public"];
    assert!(server.api_key.is_none());
    assert!(server.api_key_env.is_none());
    assert!(server.api_key_keyring.is_none());
    assert!(server.credential_source().unwrap().is_none());
}

#[tokio::test]
async fn set_server_rejects_both_api_key_and_env() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    let mut io = CapturedIo::new();
    let action = Sv {
        api_key: Some("inline-secret".into()),
        api_key_env: Some("BZR_API_KEY".into()),
        ..Sv::new("dual", "https://dual.example.com")
    }
    .into_action();
    let result = execute(
        &action,
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(matches!(err, crate::error::BzrError::InputValidation(_)));
    assert!(err.to_string().contains("at most one"));
    // Nothing was written for the rejected server.
    assert!(!config_path().exists() || !load_config().servers.contains_key("dual"));
}

#[tokio::test]
async fn set_server_table_output_reports_human_summary() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    let mut io = CapturedIo::new();
    let action = Sv {
        api_key: Some("inline-secret-key".into()),
        ..Sv::new("prod", "https://prod.example.com")
    }
    .into_action();
    execute(
        &action,
        &CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let out = io.out_str();
    assert!(out.contains("Server 'prod' configured at https://prod.example.com"));
    assert!(out.contains("Set as default server."));
    assert!(out.contains("API key source: inline config value"));
    assert!(out.contains("Config file:"));
}

#[tokio::test]
async fn set_server_tls_pin_clear_missing_server_errors() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    let mut io = CapturedIo::new();
    let action = Sv {
        tls_pin_clear: true,
        ..Sv::new("ghost", "https://ghost.example.com")
    }
    .into_action();
    let result = execute(
        &action,
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(matches!(err, crate::error::BzrError::Config(_)));
    assert!(err.to_string().contains("nothing to clear"));
}

#[tokio::test]
async fn set_server_tls_pin_clear_removes_pin_fields() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    // Seed a server that already carries a certificate pin, bypassing the
    // interactive `--tls-pin-now` probe.
    update_config_without_validation(|config| {
        config.servers.insert(
            "pinned".into(),
            ServerConfig {
                url: "https://pinned.example.com".into(),
                api_key: Some("inline-secret-key".into()),
                tls_pin_sha256: Some("AA:BB:CC".into()),
                tls_pin_issuer: Some("CN=Example".into()),
                ..ServerConfig::default()
            },
        );
        config.default_server = Some("pinned".into());
        Ok(())
    })
    .unwrap();

    let mut io = CapturedIo::new();
    let action = Sv {
        tls_pin_clear: true,
        ..Sv::new("pinned", "https://pinned.example.com")
    }
    .into_action();
    execute(
        &action,
        &CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await
    .unwrap();

    assert!(io
        .err_str()
        .contains("Certificate pin cleared for server 'pinned'."));
    let server = &load_config().servers["pinned"];
    assert!(server.tls_pin_sha256.is_none());
    assert!(server.tls_pin_issuer.is_none());
}
