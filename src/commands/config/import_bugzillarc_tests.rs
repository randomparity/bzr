#![expect(clippy::unwrap_used)]

use std::fs;
use std::path::Path;

use crate::cli::ConfigAction;
use crate::commands::config::execute;
use crate::commands::runtime::invocation::CommandContext;
use crate::config::{Config, KeyringRef, ServerConfig};
use crate::test_helpers::{load_config_unvalidated, setup_empty_config_env, CapturedIo};
use crate::types::output::OutputFormat;

use super::{default_paths, parse_sections, read_sections, resolved_servers, server_name};

#[tokio::test]
async fn import_command_persists_api_key_and_reports_unsupported_credentials() {
    let (_lock, temp) = setup_empty_config_env().await;
    let path = temp.path().join("bugzillarc");
    fs::write(
        &path,
        "[DEFAULT]\nurl=https://bugs.example.test/rest\napi_key=key\nuser=me\npassword=secret\ncert=client.pem\n",
    )
    .unwrap();
    let mut io = CapturedIo::new();

    execute(
        &ConfigAction::ImportBugzillarc { path: Some(path) },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap();

    assert!(io.out_str().contains("\"imported\": 1"));
    assert!(io
        .out_str()
        .contains("\"unsupported_password_credentials\": 1"));
    assert!(io.out_str().contains("\"unsupported_certificates\": 1"));
    let config = load_config_unvalidated();
    let server = &config.servers["bugs-example-test"];
    assert_eq!(server.url, "https://bugs.example.test/rest");
    assert_eq!(server.api_key.as_deref(), Some("key"));
    assert!(server.token.is_none());
    assert_eq!(config.default_server.as_deref(), Some("bugs-example-test"));
}

#[tokio::test]
async fn import_command_rejects_a_file_without_a_url() {
    let (_lock, temp) = setup_empty_config_env().await;
    let path = temp.path().join("bugzillarc");
    fs::write(&path, "[DEFAULT]\napi_key=key\n").unwrap();
    let mut io = CapturedIo::new();

    let result = execute(
        &ConfigAction::ImportBugzillarc { path: Some(path) },
        &CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(result.unwrap_err().to_string().contains("defines no URL"));
}

#[test]
fn explicit_missing_file_is_reported() {
    let error = read_sections(Some(Path::new("definitely-missing-bugzillarc"))).unwrap_err();

    assert!(error.to_string().contains("read bugzillarc"));
}

#[test]
fn standard_import_paths_include_the_system_file() {
    assert_eq!(
        default_paths().first(),
        Some(&std::path::PathBuf::from("/etc/bugzillarc"))
    );
}

#[test]
fn invalid_default_url_is_rejected() {
    let sections = parse_sections("[DEFAULT]\nurl=not a url\n", Path::new("fixture")).unwrap();

    assert!(resolved_servers(&sections)
        .unwrap_err()
        .to_string()
        .contains("DEFAULT url is invalid"));
}

#[test]
fn api_key_from_matching_section_is_imported() {
    let sections = parse_sections(
        "[DEFAULT]\nurl=https://bugs.example.test/rest\n[bugs.example.test]\napi_key=key\n",
        Path::new("fixture"),
    )
    .unwrap();

    let servers = resolved_servers(&sections).unwrap();

    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].api_key.as_deref(), Some("key"));
    assert!(!servers[0].has_password_credentials);
}

#[test]
fn hostname_sections_do_not_match_a_host_substring() {
    let sections = parse_sections(
        "[DEFAULT]\nurl=https://evil-bugzilla.redhat.com/rest\n[bugzilla.redhat.com]\napi_key=wrong\n[evil-bugzilla.redhat.com]\napi_key=right\n",
        Path::new("fixture"),
    )
    .unwrap();

    let servers = resolved_servers(&sections).unwrap();

    assert_eq!(servers[0].api_key.as_deref(), Some("right"));
}

#[test]
fn first_sorted_matching_section_wins() {
    let sections = parse_sections(
        "[DEFAULT]\nurl=https://bugs.example.test/rest\n[/rest]\napi_key=first\n[bugs.example.test]\napi_key=second\n",
        Path::new("fixture"),
    )
    .unwrap();

    let servers = resolved_servers(&sections).unwrap();

    assert_eq!(servers[0].api_key.as_deref(), Some("first"));
}

#[test]
fn username_and_password_are_reported_without_becoming_a_token() {
    let sections = parse_sections(
        "[DEFAULT]\nurl=https://bugs.example.test\nuser=me\npassword=secret\n",
        Path::new("fixture"),
    )
    .unwrap();

    let servers = resolved_servers(&sections).unwrap();
    let mut config = ServerConfig::default();
    servers[0].apply_to(&mut config);

    assert!(servers[0].has_password_credentials);
    assert!(config.api_key.is_none());
    assert!(config.token.is_none());
}

#[test]
fn api_key_is_preserved_when_password_credentials_are_unsupported() {
    let sections = parse_sections(
        "[DEFAULT]\nurl=https://bugs.example.test\napi_key=key\nuser=me\npassword=secret\n",
        Path::new("fixture"),
    )
    .unwrap();

    let servers = resolved_servers(&sections).unwrap();

    assert_eq!(servers[0].api_key.as_deref(), Some("key"));
    assert!(servers[0].has_password_credentials);
}

#[test]
fn later_file_values_replace_earlier_values_per_section() {
    let mut sections = parse_sections(
        "[DEFAULT]\nurl=https://bugs.example.test\napi_key=old\n",
        Path::new("system"),
    )
    .unwrap();
    let later = parse_sections("[DEFAULT]\napi_key=new\n", Path::new("user")).unwrap();
    super::merge_sections(&mut sections, later);

    let servers = resolved_servers(&sections).unwrap();

    assert_eq!(servers[0].api_key.as_deref(), Some("new"));
}

#[test]
fn matching_existing_url_updates_its_existing_alias() {
    let mut config = Config::default();
    config.servers.insert(
        "old-name".into(),
        ServerConfig {
            url: "https://bugs.example.test".into(),
            ..ServerConfig::default()
        },
    );

    assert_eq!(
        server_name(&config, "https://bugs.example.test"),
        "old-name"
    );
}

#[test]
fn new_server_name_is_sanitized_and_avoids_collisions() {
    let mut config = Config::default();
    config.servers.insert(
        "bugs-example-test".into(),
        ServerConfig {
            url: "https://other.example.test".into(),
            ..ServerConfig::default()
        },
    );

    assert_eq!(
        server_name(&config, "https://bugs.example.test"),
        "bugs-example-test-2"
    );
}

#[test]
fn importing_api_key_replaces_other_credential_sources() {
    let sections = parse_sections(
        "[DEFAULT]\nurl=https://bugs.example.test\napi_key=key\n",
        Path::new("fixture"),
    )
    .unwrap();
    let server = &resolved_servers(&sections).unwrap()[0];
    let mut config = ServerConfig {
        api_key_env: Some("BZR_API_KEY".into()),
        api_key_keyring: Some(KeyringRef::default()),
        token: Some("old-token".into()),
        ..ServerConfig::default()
    };

    server.apply_to(&mut config);

    assert_eq!(config.api_key.as_deref(), Some("key"));
    assert!(config.api_key_env.is_none());
    assert!(config.api_key_keyring.is_none());
    assert!(config.token.is_none());
}

#[test]
fn malformed_ini_is_rejected() {
    let error = parse_sections("[DEFAULT]\nnot-a-setting\n", Path::new("fixture")).unwrap_err();

    assert!(error.to_string().contains("invalid INI syntax at line 2"));
}

#[test]
fn colon_delimited_ini_settings_are_accepted() {
    let sections = parse_sections(
        "[DEFAULT]\nurl: https://bugs.example.test\napi_key: key\n",
        Path::new("fixture"),
    )
    .unwrap();

    let servers = resolved_servers(&sections).unwrap();

    assert_eq!(servers[0].api_key.as_deref(), Some("key"));
}
