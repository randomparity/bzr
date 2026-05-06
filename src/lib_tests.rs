#![expect(clippy::unwrap_used)]

use clap::Parser;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use super::*;
use crate::test_helpers::{capture_stdout, extract_json, setup_test_env};
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
    let (result, output) = capture_stdout(dispatch(&cli, OutputFormat::Json)).await;
    assert!(result.is_ok(), "dispatch failed: {result:?}");

    let parsed = extract_json(&output);
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
    let (result, output) = capture_stdout(dispatch(&cli, OutputFormat::Json)).await;
    assert!(result.is_ok(), "dispatch failed: {result:?}");

    let parsed = extract_json(&output);
    assert_eq!(parsed["name"], "firefox-new");
    assert_eq!(parsed["action"], "saved");
}
