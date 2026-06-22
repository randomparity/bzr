#![expect(clippy::unwrap_used)]

use clap::Parser;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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
    assert!(!commands::runtime::dry_run::enabled());
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

#[tokio::test]
async fn dispatch_allows_dry_run_on_admin_create() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("POST"))
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
        "product",
        "create",
        "--name",
        "DryRun",
        "--description",
        "Preview",
    ])
    .unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "dry-run dispatch failed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["resource"], "product");
    assert_eq!(parsed["action"], "dry-run");
}

#[tokio::test]
async fn dispatch_rejects_dry_run_on_group_membership_mutation() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let cli = cli::Cli::try_parse_from([
        "bzr",
        "--server",
        "test",
        "--dry-run",
        "group",
        "add-user",
        "--group",
        "editbugs",
        "--user",
        "alice@example.com",
    ])
    .unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(matches!(
        result,
        Err(error::BzrError::InputValidation(ref msg)) if msg.contains("product create")
            && msg.contains("group create/update")
    ));
}

#[tokio::test]
async fn dispatch_rejects_write_without_credentials_before_network() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    write_public_config(&tmp, &mock.uri());

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from([
        "bzr", "--server", "public", "comment", "add", "1", "--body", "test",
    ])
    .unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(matches!(
        result,
        Err(error::BzrError::Config(ref msg))
            if msg.contains("comment add") && msg.contains("credentials")
    ));
}

#[tokio::test]
async fn dispatch_rejects_bug_my_without_credentials() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    write_public_config(&tmp, &mock.uri());

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from(["bzr", "--server", "public", "bug", "my"]).unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(matches!(
        result,
        Err(error::BzrError::Config(ref msg))
            if msg.contains("bug my") && msg.contains("credentials")
    ));
}

#[tokio::test]
async fn dispatch_rejects_whoami_without_credentials() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    write_public_config(&tmp, &mock.uri());

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from(["bzr", "--server", "public", "whoami"]).unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(matches!(
        result,
        Err(error::BzrError::Config(ref msg))
            if msg.contains("whoami") && msg.contains("credentials")
    ));
}

#[tokio::test]
async fn dispatch_rejects_inline_write_without_api_key_env_before_network() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;

    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from([
        "bzr",
        "--server-url",
        mock.uri().as_str(),
        "comment",
        "add",
        "1",
        "--body",
        "test",
    ])
    .unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(matches!(
        result,
        Err(error::BzrError::Config(ref msg))
            if msg.contains("comment add") && msg.contains("--server-api-key-env")
    ));
}

#[tokio::test]
async fn dispatch_allows_public_server_info_without_credentials() {
    let _lock = ENV_LOCK.lock().await;
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    write_public_config(&tmp, &mock.uri());

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/extensions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"extensions": {}})),
        )
        .mount(&mock)
        .await;

    let cli = cli::Cli::try_parse_from(["bzr", "--server", "public", "--json", "server", "info"])
        .unwrap();
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = dispatch(&cli, OutputFormat::Json, &mut io.writers()).await;

    assert!(result.is_ok(), "public server info failed: {result:?}");
}

fn write_public_config(tmp: &tempfile::TempDir, server_url: &str) {
    let config_dir = tmp.path().join("bzr");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_content = format!(
        r#"
default_server = "public"

[servers.public]
url = "{server_url}"
"#,
    );
    std::fs::write(config_dir.join("config.toml"), config_content).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        std::fs::set_permissions(&config_dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(
            config_dir.join("config.toml"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
    }
    // SAFETY: Tests are serialized via ENV_LOCK.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };
}
