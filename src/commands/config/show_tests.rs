#![expect(clippy::unwrap_used)]

//! Direct tests for the `config show` leaf. Read-only, local-only command.

use std::path::Path;

use crate::cli::ConfigAction;
use crate::commands::config::execute;
use crate::commands::runtime::invocation::CommandContext;
use crate::test_helpers::{
    run_config_action_json_at, seed_inline_server_at, setup_empty_isolated_env, CapturedIo,
};
use crate::types::output::OutputFormat;
use crate::types::transport::AuthMethod;

/// A command context pinned to an explicit config path, so config resolution
/// never consults `XDG_CONFIG_HOME` and the test needs no `ENV_LOCK`
/// (ADR-0002).
fn ctx_at(config_path: &Path, format: OutputFormat) -> CommandContext {
    CommandContext::new(None, format, None)
        .with_config_path_override(Some(config_path.to_path_buf()))
}

#[tokio::test]
async fn show_json_includes_populated_server_details() {
    let (_tmp, config_path) = setup_empty_isolated_env();
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
        &ctx_at(&config_path, OutputFormat::Json),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let parsed = run_config_action_json_at(&config_path, ConfigAction::Show).await;
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
    let (_tmp, config_path) = setup_empty_isolated_env();
    let parsed = run_config_action_json_at(&config_path, ConfigAction::Show).await;
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
    let (_tmp, config_path) = setup_empty_isolated_env();
    let mut io = CapturedIo::new();
    execute(
        &ConfigAction::Show,
        &ctx_at(&config_path, OutputFormat::Table),
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
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(
        &config_path,
        "prod",
        "https://prod.example.com",
        "abcdef1234567890",
    )
    .await;

    let mut io = CapturedIo::new();
    execute(
        &ConfigAction::Show,
        &ctx_at(&config_path, OutputFormat::Table),
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
