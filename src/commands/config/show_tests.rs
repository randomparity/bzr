#![expect(clippy::unwrap_used)]

//! Direct tests for the `config show` leaf. Read-only, local-only command.

use crate::cli::ConfigAction;
use crate::commands::config::execute;
use crate::commands::runtime::invocation::CommandContext;
use crate::test_helpers::{
    run_config_action_json, seed_inline_server, setup_empty_config_env, CapturedIo,
};
use crate::types::output::OutputFormat;
use crate::types::transport::AuthMethod;

#[tokio::test]
async fn show_json_includes_populated_server_details() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    let mut io = CapturedIo::new();
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
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let parsed = run_config_action_json(ConfigAction::Show).await;
    assert_eq!(parsed["default_server"], "prod");
    assert_eq!(parsed["servers"]["prod"]["url"], "https://prod.example.com");
    assert_eq!(parsed["servers"]["prod"]["email"], "admin@example.com");
    assert_eq!(parsed["servers"]["prod"]["auth_method"], "header");
    assert_eq!(parsed["servers"]["prod"]["tls_insecure"], true);
    // The inline API key is redacted to a prefix, never shown in full.
    assert_eq!(parsed["servers"]["prod"]["api_key"], "abcdef12...");
    assert_eq!(parsed["servers"]["prod"]["api_key_source"], "inline");
}

#[tokio::test]
async fn show_empty_config_json_has_no_servers() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    let parsed = run_config_action_json(ConfigAction::Show).await;
    assert!(parsed["default_server"].is_null());
    assert!(
        parsed["servers"]
            .as_object()
            .is_none_or(serde_json::Map::is_empty),
        "expected no servers in an empty config: {parsed}"
    );
}

#[tokio::test]
async fn show_empty_config_table_reports_no_servers() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    let mut io = CapturedIo::new();
    execute(
        &ConfigAction::Show,
        &CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let out = io.out_str();
    assert!(out.contains("Config file:"));
    assert!(out.contains("No servers configured."));
}

#[tokio::test]
async fn show_table_lists_configured_server() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("prod", "https://prod.example.com", "abcdef1234567890").await;

    let mut io = CapturedIo::new();
    execute(
        &ConfigAction::Show,
        &CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let out = io.out_str();
    assert!(out.contains("Default server: prod"));
    assert!(out.contains("https://prod.example.com"));
    assert!(
        !out.contains("abcdef1234567890"),
        "the full API key must never appear in table output: {out}"
    );
}
