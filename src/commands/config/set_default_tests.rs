#![expect(clippy::unwrap_used)]

//! Direct tests for the `config set-default` leaf. Local-only command.

use std::path::Path;

use crate::cli::ConfigAction;
use crate::commands::config::execute;
use crate::commands::runtime::invocation::CommandContext;
use crate::error::BzrError;
use crate::test_helpers::{
    load_config_at, run_config_action_json_at, seed_inline_server_at, setup_empty_isolated_env,
    CapturedIo,
};
use crate::types::output::OutputFormat;

/// A command context pinned to an explicit config path, so config resolution
/// never consults `XDG_CONFIG_HOME` and the test needs no `ENV_LOCK`
/// (ADR-0002).
fn ctx_at(config_path: &Path, format: OutputFormat) -> CommandContext {
    CommandContext::new(None, format, None)
        .with_config_path_override(Some(config_path.to_path_buf()))
}

#[tokio::test]
async fn set_default_on_empty_config_errors() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::SetDefault {
            name: "nonexistent".into(),
        },
        &ctx_at(&config_path, OutputFormat::Table),
        &mut io.writers(),
    )
    .await;
    assert!(matches!(result.unwrap_err(), BzrError::Config(_)));
}

#[tokio::test]
async fn set_default_persists_selected_server() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    for (name, url) in [
        ("first", "https://first.example.com"),
        ("second", "https://second.example.com"),
    ] {
        seed_inline_server_at(&config_path, name, url, &format!("{name}-key-1234567890")).await;
    }

    let json = run_config_action_json_at(
        &config_path,
        ConfigAction::SetDefault {
            name: "second".into(),
        },
    )
    .await;
    assert_eq!(json["name"], "second");
    assert_eq!(json["action"], "updated");
    assert_eq!(json["resource"], "server");
    assert_eq!(
        load_config_at(&config_path).default_server.as_deref(),
        Some("second")
    );
}

#[tokio::test]
async fn set_default_to_current_default_is_idempotent() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "only", "https://only.example.com", "k").await;
    // "only" is already the default (first added); re-setting it succeeds.
    let json = run_config_action_json_at(
        &config_path,
        ConfigAction::SetDefault {
            name: "only".into(),
        },
    )
    .await;
    assert_eq!(json["action"], "updated");
    assert_eq!(
        load_config_at(&config_path).default_server.as_deref(),
        Some("only")
    );
}

#[tokio::test]
async fn set_default_table_output_reports_human_summary() {
    let (_tmp, config_path) = setup_empty_isolated_env();
    seed_inline_server_at(&config_path, "prod", "https://prod.example.com", "k").await;
    seed_inline_server_at(&config_path, "stage", "https://stage.example.com", "k").await;

    let mut io = CapturedIo::new();
    execute(
        &ConfigAction::SetDefault {
            name: "stage".into(),
        },
        &ctx_at(&config_path, OutputFormat::Table),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let out = io.out_str();
    assert!(out.contains("Default server set to 'stage'"));
    assert!(out.contains("Config file:"));
}
