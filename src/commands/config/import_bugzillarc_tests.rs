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
fn explicit_url_sections_import_without_a_default_url() {
    let sections = parse_sections(
        "[DEFAULT]\napi_key=default-key\n[https://bugs.example.test/rest]\napi_key=first\n[https://second.example.test]\napi_key=second\n",
        Path::new("fixture"),
    )
    .unwrap();

    let servers = resolved_servers(&sections).unwrap();

    assert_eq!(servers.len(), 2);
    assert_eq!(servers[0].url, "https://bugs.example.test/rest");
    assert_eq!(servers[0].api_key.as_deref(), Some("first"));
    assert_eq!(servers[1].url, "https://second.example.test");
    assert_eq!(servers[1].api_key.as_deref(), Some("second"));
}

#[test]
fn section_only_import_does_not_inherit_default_credentials() {
    let sections = parse_sections(
        "[DEFAULT]\napi_key=shared\nuser=fixture-user\npassword=fixture-password\n[https://bugs.example.test]\n[https://second.example.test]\napi_key=second\n",
        Path::new("fixture"),
    )
    .unwrap();

    let servers = resolved_servers(&sections).unwrap();

    assert_eq!(servers.len(), 2);
    assert!(servers[0].api_key.is_none());
    assert!(!servers[0].has_password_credentials);
    assert_eq!(servers[1].api_key.as_deref(), Some("second"));
}

#[test]
fn section_only_import_ignores_non_url_section_names() {
    let sections = parse_sections(
        "[bugs.example.test]\napi_key=wrong\n[https://bugs.example.test]\napi_key=right\n",
        Path::new("fixture"),
    )
    .unwrap();

    let servers = resolved_servers(&sections).unwrap();

    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].api_key.as_deref(), Some("right"));
}

#[test]
fn section_only_import_reports_how_to_resolve_non_url_sections() {
    let sections =
        parse_sections("[bugs.example.test]\napi_key=key\n", Path::new("fixture")).unwrap();

    assert!(resolved_servers(&sections).unwrap().is_empty());
}

#[test]
fn section_only_import_rejects_malformed_explicit_url_sections() {
    let sections =
        parse_sections("[https://bad host]\napi_key=key\n", Path::new("fixture")).unwrap();

    assert!(resolved_servers(&sections)
        .unwrap_err()
        .to_string()
        .contains("section URL is invalid"));
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
fn hostname_sections_match_the_exact_port() {
    let sections = parse_sections(
        "[DEFAULT]\nurl=https://bugs.example.test:8443/rest\n[bugs.example.test]\napi_key=wrong\n[bugs.example.test:8444]\napi_key=also-wrong\n[bugs.example.test:8443]\napi_key=right\n",
        Path::new("fixture"),
    )
    .unwrap();

    let servers = resolved_servers(&sections).unwrap();

    assert_eq!(servers[0].api_key.as_deref(), Some("right"));
}

#[test]
fn hostname_sections_preserve_explicit_default_ports() {
    let https = parse_sections(
        "[DEFAULT]\nurl=https://bugs.example.test:443/rest\n[bugs.example.test]\napi_key=wrong\n[bugs.example.test:443]\napi_key=right\n",
        Path::new("fixture"),
    )
    .unwrap();
    let http = parse_sections(
        "[DEFAULT]\nurl=http://bugs.example.test:80/rest\n[bugs.example.test]\napi_key=wrong\n[bugs.example.test:80]\napi_key=right\n",
        Path::new("fixture"),
    )
    .unwrap();

    assert_eq!(
        resolved_servers(&https).unwrap()[0].api_key.as_deref(),
        Some("right")
    );
    assert_eq!(
        resolved_servers(&http).unwrap()[0].api_key.as_deref(),
        Some("right")
    );

    let uppercase_scheme = parse_sections(
        "[DEFAULT]\nurl=HTTPS://bugs.example.test:443/rest\n[bugs.example.test]\napi_key=wrong\n[bugs.example.test:443]\napi_key=right\n",
        Path::new("fixture"),
    )
    .unwrap();
    assert_eq!(
        resolved_servers(&uppercase_scheme).unwrap()[0]
            .api_key
            .as_deref(),
        Some("right")
    );
}

#[test]
fn hostname_sections_preserve_userinfo() {
    let sections = parse_sections(
        "[DEFAULT]\nurl=https://user:password@bugs.example.test/rest\n[bugs.example.test]\napi_key=wrong\n[user:password@bugs.example.test]\napi_key=right\n",
        Path::new("fixture"),
    )
    .unwrap();

    let servers = resolved_servers(&sections).unwrap();

    assert_eq!(servers[0].api_key.as_deref(), Some("right"));
}

#[test]
fn hostname_sections_preserve_default_ports_with_userinfo_and_ipv6() {
    let userinfo = parse_sections(
        "[DEFAULT]\nurl=https://user:password@bugs.example.test:443/rest\n[user:password@bugs.example.test]\napi_key=wrong\n[user:password@bugs.example.test:443]\napi_key=right\n",
        Path::new("fixture"),
    )
    .unwrap();
    let ipv6 = parse_sections(
        "[DEFAULT]\nurl=http://[2001:db8::1]:80/rest\n[[2001:db8::1]]\napi_key=wrong\n[[2001:db8::1]:80]\napi_key=right\n",
        Path::new("fixture"),
    )
    .unwrap();

    assert_eq!(
        resolved_servers(&userinfo).unwrap()[0].api_key.as_deref(),
        Some("right")
    );
    assert_eq!(
        resolved_servers(&ipv6).unwrap()[0].api_key.as_deref(),
        Some("right")
    );
}

#[test]
fn hostname_sections_preserve_ipv6_brackets() {
    let sections = parse_sections(
        "[DEFAULT]\nurl=https://[2001:db8::1]:8443/rest\n[[2001:db8::1]]\napi_key=wrong\n[[2001:db8::1]:8443]\napi_key=right\n",
        Path::new("fixture"),
    )
    .unwrap();

    let servers = resolved_servers(&sections).unwrap();

    assert_eq!(servers[0].api_key.as_deref(), Some("right"));
}

#[test]
fn default_url_import_uses_only_its_exact_authority_section() {
    let sections = parse_sections(
        "[DEFAULT]\nurl=https://bugs.example.test/rest\n[/rest]\napi_key=first\n[bugs.example.test]\napi_key=second\n",
        Path::new("fixture"),
    )
    .unwrap();

    let servers = resolved_servers(&sections).unwrap();

    assert_eq!(servers[0].api_key.as_deref(), Some("second"));
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
