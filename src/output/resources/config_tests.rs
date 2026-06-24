#![expect(clippy::unwrap_used)]

use super::*;
use crate::config::{Config, ServerConfig};
use std::collections::HashMap;
use std::path::Path;

#[test]
fn config_view_json_serialization() {
    let view = ConfigView {
        config_file: "/etc/bzr/config.toml".into(),
        default_server: Some("prod".into()),
        servers: std::collections::BTreeMap::new(),
    };
    let json: serde_json::Value = serde_json::to_value(&view).unwrap();
    assert_eq!(json["config_file"], "/etc/bzr/config.toml");
    assert_eq!(json["default_server"], "prod");
}

#[test]
fn config_view_no_default_server() {
    let view = ConfigView {
        config_file: "/tmp/config.toml".into(),
        default_server: None,
        servers: std::collections::BTreeMap::new(),
    };
    let json: serde_json::Value = serde_json::to_value(&view).unwrap();
    assert!(json["default_server"].is_null());
}

#[test]
fn auth_display_formats_detected_and_undetected_states() {
    assert_eq!(auth_display(None), "auto (not yet detected)");
    assert_eq!(auth_display(Some(&AuthMethod::Header)), "header");
}

#[test]
fn config_view_from_config_masks_key_and_includes_flags() {
    let mut servers = HashMap::new();
    servers.insert(
        "prod".into(),
        ServerConfig {
            url: "https://bugzilla.example".into(),
            email: Some("admin@example.com".into()),
            api_key: Some("1234567890abcdef".into()),
            api_key_env: None,
            api_key_keyring: None,
            auth_method: Some(AuthMethod::Header),
            api_mode: None,
            server_version: None,
            tls_insecure: true,
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_issuer: None,
            tls_pin_issuer_der: None,
        },
    );
    let config = Config {
        default_server: Some("prod".into()),
        servers,
        queries: HashMap::new(),
        templates: HashMap::new(),
    };

    let view = ConfigView::from_config(&config, Path::new("/tmp/bzr/config.toml"));
    let server = &view.servers["prod"];
    let json: serde_json::Value = serde_json::to_value(server).unwrap();

    assert_eq!(json["url"], "https://bugzilla.example");
    assert_eq!(json["email"], "admin@example.com");
    assert_eq!(json["api_key"], "12345678...");
    assert_eq!(json["api_key_source"], "inline");
    assert_eq!(json["auth_method"], "header");
    assert_eq!(json["tls_insecure"], true);
}

#[test]
fn config_view_from_config_shows_env_backed_keys_without_resolving() {
    let mut servers = HashMap::new();
    servers.insert(
        "prod".into(),
        ServerConfig {
            url: "https://bugzilla.example".into(),
            email: None,
            api_key: None,
            api_key_env: Some("BZR_API_KEY".into()),
            api_key_keyring: None,
            auth_method: None,
            api_mode: None,
            server_version: None,
            tls_insecure: false,
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_issuer: None,
            tls_pin_issuer_der: None,
        },
    );
    let config = Config {
        default_server: Some("prod".into()),
        servers,
        queries: HashMap::new(),
        templates: HashMap::new(),
    };

    let view = ConfigView::from_config(&config, Path::new("/tmp/bzr/config.toml"));
    let server = &view.servers["prod"];
    let json: serde_json::Value = serde_json::to_value(server).unwrap();

    assert_eq!(json["api_key"], "BZR_API_KEY");
    assert_eq!(json["api_key_source"], "env");
}

#[test]
fn config_view_displays_missing_api_key_source_as_none() {
    let mut servers = HashMap::new();
    servers.insert(
        "public".into(),
        ServerConfig {
            url: "https://bugzilla.example".into(),
            email: None,
            api_key: None,
            api_key_env: None,
            api_key_keyring: None,
            auth_method: None,
            api_mode: None,
            server_version: None,
            tls_insecure: false,
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_issuer: None,
            tls_pin_issuer_der: None,
        },
    );
    let config = Config {
        default_server: Some("public".into()),
        servers,
        queries: HashMap::new(),
        templates: HashMap::new(),
    };

    let view = ConfigView::from_config(&config, Path::new("/tmp/bzr/config.toml"));
    let server = &view.servers["public"];
    let json: serde_json::Value = serde_json::to_value(server).unwrap();

    assert_eq!(json["api_key"], "none");
    assert_eq!(json["api_key_source"], "none");
}

#[test]
fn server_display_info_keyring_source() {
    let srv = ServerConfig {
        url: "https://example.com".into(),
        api_key: None,
        api_key_env: None,
        api_key_keyring: Some(crate::config::KeyringRef {
            service: Some("bzr".into()),
            account: Some("prod".into()),
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };
    let info = ServerDisplayInfo::from_config(&srv);
    assert_eq!(info.api_key_source, DisplayCredentialSource::Keyring);
    assert!(info.api_key.contains("bzr"));
    assert!(info.api_key.contains("prod"));
}

#[test]
fn display_server_with_tls_pin() {
    let mut servers = HashMap::new();
    servers.insert(
        "pinned".into(),
        ServerConfig {
            url: "https://bugzilla.example".into(),
            email: None,
            api_key: Some("1234567890abcdef".into()),
            api_key_env: None,
            api_key_keyring: None,
            auth_method: Some(AuthMethod::Header),
            api_mode: None,
            server_version: None,
            tls_insecure: false,
            tls_ca_cert: None,
            tls_pin_sha256: Some("sha256//abc123".into()),
            tls_pin_issuer: Some("CN=Test CA".into()),
            tls_pin_issuer_der: None,
        },
    );
    let config = Config {
        default_server: Some("pinned".into()),
        servers,
        queries: HashMap::new(),
        templates: HashMap::new(),
    };

    let view = ConfigView::from_config(&config, Path::new("/tmp/bzr/config.toml"));
    let server = &view.servers["pinned"];
    let json: serde_json::Value = serde_json::to_value(server).unwrap();

    assert_eq!(json["tls_pin"], "sha256//abc123 (CN=Test CA)");
    assert!(json.get("tls_insecure").is_none());
}

// ── DisplayCredentialSource::as_str ──────────────────────────────

#[test]
fn display_credential_source_as_str_maps_each_variant_to_its_label() {
    // Kill `-> &'static str with ""`/`"xyzzy"` mutants: assert each variant
    // returns the exact expected label string.
    assert_eq!(DisplayCredentialSource::Inline.as_str(), "inline");
    assert_eq!(DisplayCredentialSource::Env.as_str(), "env");
    assert_eq!(DisplayCredentialSource::Keyring.as_str(), "keyring");
    assert_eq!(DisplayCredentialSource::None.as_str(), "none");
    assert_eq!(DisplayCredentialSource::Invalid.as_str(), "invalid");

    // Confirm all five are pairwise-distinct so a single wrong constant
    // can't accidentally satisfy two variant checks.
    let labels = [
        DisplayCredentialSource::Inline.as_str(),
        DisplayCredentialSource::Env.as_str(),
        DisplayCredentialSource::Keyring.as_str(),
        DisplayCredentialSource::None.as_str(),
        DisplayCredentialSource::Invalid.as_str(),
    ];
    let unique: std::collections::HashSet<_> = labels.iter().collect();
    assert_eq!(unique.len(), 5, "all five as_str labels must be distinct");
}

fn make_display_info(
    url: &str,
    api_key: &str,
    source: DisplayCredentialSource,
    tls_insecure: bool,
) -> ServerDisplayInfo {
    ServerDisplayInfo {
        url: url.into(),
        email: None,
        api_key: api_key.into(),
        api_key_source: source,
        auth_method: None,
        tls_insecure,
        tls_ca_cert: None,
        tls_pin: None,
    }
}

fn capture_write_config(view: &ConfigView) -> String {
    let mut buf = Vec::new();
    write_config(view, OutputFormat::Table, &mut buf);
    String::from_utf8(buf).unwrap()
}

#[test]
fn write_config_renders_table_for_inline_server() {
    // Exercises write_config / write_server / write_api_key with the
    // inline credential branch.
    let mut info = make_display_info(
        "https://bugzilla.example",
        "12345678...",
        DisplayCredentialSource::Inline,
        true,
    );
    info.email = Some("admin@example.com".into());
    info.auth_method = Some(AuthMethod::Header);
    let mut servers = std::collections::BTreeMap::new();
    servers.insert("prod".into(), info);
    let view = ConfigView {
        config_file: "/tmp/bzr/config.toml".into(),
        default_server: Some("prod".into()),
        servers,
    };
    let output = capture_write_config(&view);
    for needle in [
        "Config file: /tmp/bzr/config.toml",
        "Default server: prod",
        "[prod]",
        "https://bugzilla.example",
        "admin@example.com",
        "API Key",
        "12345678...",
        "insecure",
    ] {
        assert!(output.contains(needle), "missing {needle:?} in output");
    }
}

#[test]
fn write_config_renders_env_and_keyring_labels() {
    // Exercises both non-inline branches of write_api_key in one run.
    let mut servers = std::collections::BTreeMap::new();
    servers.insert(
        "env-srv".into(),
        make_display_info(
            "https://env.example",
            "BZR_API_KEY",
            DisplayCredentialSource::Env,
            false,
        ),
    );
    servers.insert(
        "kr-srv".into(),
        make_display_info(
            "https://kr.example",
            "bzr/kr-srv",
            DisplayCredentialSource::Keyring,
            false,
        ),
    );
    let view = ConfigView {
        config_file: "/tmp/bzr/config.toml".into(),
        default_server: None,
        servers,
    };
    let output = capture_write_config(&view);
    for needle in ["API Key Env", "BZR_API_KEY", "Keyring", "bzr/kr-srv"] {
        assert!(output.contains(needle), "missing {needle:?} in output");
    }
}

#[test]
fn write_config_renders_empty_servers_message() {
    let view = ConfigView {
        config_file: "/tmp/bzr/config.toml".into(),
        default_server: None,
        servers: std::collections::BTreeMap::new(),
    };
    let output = capture_write_config(&view);
    assert!(output.contains("No servers configured."));
}

#[test]
fn server_display_info_keyring_source_default_account() {
    // account=None should render with `<server-name>` placeholder per the
    // Task 3 implementation.
    let srv = ServerConfig {
        url: "https://example.com".into(),
        api_key: None,
        api_key_env: None,
        api_key_keyring: Some(crate::config::KeyringRef {
            service: None,
            account: None,
        }),
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
        tls_pin_issuer_der: None,
    };
    let info = ServerDisplayInfo::from_config(&srv);
    assert_eq!(info.api_key_source, DisplayCredentialSource::Keyring);
    assert!(info.api_key.contains("bzr"));
    // The display shows the placeholder, not a real server name (since
    // ServerDisplayInfo doesn't know the server name — that's higher
    // up in ConfigView).
    assert!(info.api_key.contains("<server-name>"));
}
