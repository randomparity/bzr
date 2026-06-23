#![expect(clippy::unwrap_used)]

//! Direct tests for the `config set-default` leaf. Local-only command.

use crate::cli::ConfigAction;
use crate::commands::config::execute;
use crate::commands::runtime::context::CommandContext;
use crate::error::BzrError;
use crate::test_helpers::{
    load_config, run_config_action_json, seed_inline_server, setup_empty_config_env, CapturedIo,
};
use crate::types::common::OutputFormat;

#[tokio::test]
async fn set_default_on_empty_config_errors() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    let mut io = CapturedIo::new();
    let result = execute(
        &ConfigAction::SetDefault {
            name: "nonexistent".into(),
        },
        &CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await;
    assert!(matches!(result.unwrap_err(), BzrError::Config(_)));
}

#[tokio::test]
async fn set_default_persists_selected_server() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    for (name, url) in [
        ("first", "https://first.example.com"),
        ("second", "https://second.example.com"),
    ] {
        seed_inline_server(name, url, &format!("{name}-key-1234567890")).await;
    }

    let json = run_config_action_json(ConfigAction::SetDefault {
        name: "second".into(),
    })
    .await;
    assert_eq!(json["name"], "second");
    assert_eq!(json["action"], "updated");
    assert_eq!(json["resource"], "server");
    assert_eq!(load_config().default_server.as_deref(), Some("second"));
}

#[tokio::test]
async fn set_default_to_current_default_is_idempotent() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("only", "https://only.example.com", "k").await;
    // "only" is already the default (first added); re-setting it succeeds.
    let json = run_config_action_json(ConfigAction::SetDefault {
        name: "only".into(),
    })
    .await;
    assert_eq!(json["action"], "updated");
    assert_eq!(load_config().default_server.as_deref(), Some("only"));
}

#[tokio::test]
async fn set_default_table_output_reports_human_summary() {
    let (_lock, _tmp) = setup_empty_config_env().await;
    seed_inline_server("prod", "https://prod.example.com", "k").await;
    seed_inline_server("stage", "https://stage.example.com", "k").await;

    let mut io = CapturedIo::new();
    execute(
        &ConfigAction::SetDefault {
            name: "stage".into(),
        },
        &CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await
    .unwrap();

    let out = io.out_str();
    assert!(out.contains("Default server set to 'stage'"));
    assert!(out.contains("Config file:"));
}
