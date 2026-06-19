#![expect(clippy::unwrap_used)]

use clap::Parser;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use super::*;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

#[tokio::test]
async fn dispatch_whoami_defaults_to_show_action() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1,
            "name": "admin@example.com",
            "real_name": "Admin User"
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from(["bzr", "--server", "test", "--json", "whoami"]).unwrap();
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut __io.writers()).await;
    let output = __io.out_str().to_string();
    assert!(result.is_ok(), "dispatch failed: {result:?}");

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["name"], "admin@example.com");
}

#[tokio::test]
async fn dispatch_routes_local_query_commands() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let cli = cli::Cli::try_parse_from([
        "bzr",
        "--json",
        "query",
        "save",
        "firefox-new",
        "--product",
        "Firefox",
    ])
    .unwrap();
    let mut __io2 = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut __io2.writers()).await;
    let output = __io2.out_str().to_string();
    assert!(result.is_ok(), "dispatch failed: {result:?}");

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["name"], "firefox-new");
    assert_eq!(parsed["action"], "saved");
}

#[tokio::test]
async fn dispatch_rejects_dry_run_on_unsupported_command() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    // --dry-run on a non-mutation command is rejected before any network I/O,
    // so a silently-ignored preview can never turn into a real write.
    let cli = cli::Cli::try_parse_from(["bzr", "--server", "test", "--dry-run", "whoami"]).unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(matches!(
        result,
        Err(error::BzrError::InputValidation(ref msg)) if msg.contains("--dry-run")
    ));
    // The reject must not leak dry-run state into a later command.
    assert!(!commands::dry_run::enabled());
}

#[tokio::test]
async fn dispatch_allows_dry_run_on_bug_update() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from([
        "bzr",
        "--server",
        "test",
        "--json",
        "--dry-run",
        "bug",
        "update",
        "5",
        "--status",
        "RESOLVED",
    ])
    .unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "dry-run dispatch failed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([5]));
}
