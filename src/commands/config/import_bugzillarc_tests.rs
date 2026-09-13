#![expect(clippy::unwrap_used)]

use std::path::Path;

use crate::config::{Config, ServerConfig};

use super::{parse_sections, resolved_servers, server_name};

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
