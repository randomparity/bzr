use std::io::{self, Write};

use serde::Serialize;

use super::formatting::{mask_api_key, print_field, print_formatted, print_optional_field};
use crate::config::{CredentialSource, CredentialSourceKind};
use crate::types::{AuthMethod, OutputFormat};

fn auth_display(auth_method: Option<&AuthMethod>) -> String {
    auth_method.map_or_else(
        || "auto (not yet detected)".to_string(),
        ToString::to_string,
    )
}

#[derive(Serialize)]
#[non_exhaustive]
pub struct ServerDisplayInfo {
    url: String,
    email: Option<String>,
    api_key: String,
    api_key_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_method: Option<AuthMethod>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    tls_insecure: bool,
}

impl ServerDisplayInfo {
    fn from_config(srv: &crate::config::ServerConfig) -> Self {
        let (api_key, api_key_source) = match srv.credential_source() {
            Ok(CredentialSource::Inline(api_key)) => {
                (mask_api_key(api_key), CredentialSourceKind::Inline.as_str())
            }
            Ok(CredentialSource::EnvVar(var_name)) => {
                (var_name.to_string(), CredentialSourceKind::Env.as_str())
            }
            Ok(CredentialSource::Keyring { service, account }) => {
                let display = if account.is_empty() {
                    format!("{service}/<server-name>")
                } else {
                    format!("{service}/{account}")
                };
                (display, CredentialSourceKind::Keyring.as_str())
            }
            Err(_) => ("[invalid config]".to_string(), "invalid"),
        };
        Self {
            url: srv.url.clone(),
            email: srv.email.clone(),
            api_key,
            api_key_source: api_key_source.to_string(),
            auth_method: srv.auth_method,
            tls_insecure: srv.tls_insecure,
        }
    }
}

#[derive(Serialize)]
#[non_exhaustive]
pub struct ConfigView {
    pub config_file: String,
    pub default_server: Option<String>,
    pub servers: std::collections::BTreeMap<String, ServerDisplayInfo>,
}

impl ConfigView {
    pub fn from_config(config: &crate::config::Config, path: &std::path::Path) -> Self {
        let servers = config
            .servers
            .iter()
            .map(|(name, srv)| (name.clone(), ServerDisplayInfo::from_config(srv)))
            .collect();
        Self {
            config_file: path.to_string_lossy().into_owned(),
            default_server: config.default_server.clone(),
            servers,
        }
    }
}

fn print_api_key(s: &ServerDisplayInfo) {
    let label = match s.api_key_source.as_str() {
        "env" => "API Key Env",
        "keyring" => "Keyring",
        _ => "API Key",
    };
    print_field(label, &s.api_key);
}

fn print_server(name: &str, s: &ServerDisplayInfo) {
    writeln!(io::stdout(), "\n[{name}]").expect("write to output");
    print_field("URL", &s.url);
    print_optional_field("Email", s.email.as_deref());
    print_api_key(s);
    print_field("API Key Source", &s.api_key_source);
    print_field("Auth", &auth_display(s.auth_method.as_ref()));
    if s.tls_insecure {
        print_field("TLS", "insecure (certificate verification disabled)");
    }
}

pub fn print_config(view: &ConfigView, format: OutputFormat) {
    print_formatted(view, format, |v| {
        let mut out = io::stdout();
        writeln!(out, "Config file: {}\n", v.config_file).expect("write to output");
        if let Some(ref def) = v.default_server {
            writeln!(out, "Default server: {def}").expect("write to output");
        }
        if v.servers.is_empty() {
            writeln!(out, "No servers configured.").expect("write to output");
            return;
        }
        for (name, s) in &v.servers {
            print_server(name, s);
        }
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
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
        };
        let info = ServerDisplayInfo::from_config(&srv);
        assert_eq!(info.api_key_source, "keyring");
        assert!(info.api_key.contains("bzr"));
        assert!(info.api_key.contains("prod"));
    }

    fn make_display_info(
        url: &str,
        api_key: &str,
        source: &str,
        tls_insecure: bool,
    ) -> ServerDisplayInfo {
        ServerDisplayInfo {
            url: url.into(),
            email: None,
            api_key: api_key.into(),
            api_key_source: source.into(),
            auth_method: None,
            tls_insecure,
        }
    }

    async fn capture_print_config(view: &ConfigView) -> String {
        let ((), output) = crate::test_helpers::capture_stdout(async {
            print_config(view, OutputFormat::Table);
        })
        .await;
        output
    }

    #[tokio::test]
    async fn print_config_renders_table_for_inline_server() {
        // Exercises print_config / print_server / print_api_key with the
        // inline credential branch.
        let _lock = crate::ENV_LOCK.lock().await;
        let mut info = make_display_info("https://bugzilla.example", "12345678...", "inline", true);
        info.email = Some("admin@example.com".into());
        info.auth_method = Some(AuthMethod::Header);
        let mut servers = std::collections::BTreeMap::new();
        servers.insert("prod".into(), info);
        let view = ConfigView {
            config_file: "/tmp/bzr/config.toml".into(),
            default_server: Some("prod".into()),
            servers,
        };
        let output = capture_print_config(&view).await;
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

    #[tokio::test]
    async fn print_config_renders_env_and_keyring_labels() {
        // Exercises both non-inline branches of print_api_key in one run.
        let _lock = crate::ENV_LOCK.lock().await;
        let mut servers = std::collections::BTreeMap::new();
        servers.insert(
            "env-srv".into(),
            make_display_info("https://env.example", "BZR_API_KEY", "env", false),
        );
        servers.insert(
            "kr-srv".into(),
            make_display_info("https://kr.example", "bzr/kr-srv", "keyring", false),
        );
        let view = ConfigView {
            config_file: "/tmp/bzr/config.toml".into(),
            default_server: None,
            servers,
        };
        let output = capture_print_config(&view).await;
        for needle in ["API Key Env", "BZR_API_KEY", "Keyring", "bzr/kr-srv"] {
            assert!(output.contains(needle), "missing {needle:?} in output");
        }
    }

    #[tokio::test]
    async fn print_config_renders_empty_servers_message() {
        let _lock = crate::ENV_LOCK.lock().await;
        let view = ConfigView {
            config_file: "/tmp/bzr/config.toml".into(),
            default_server: None,
            servers: std::collections::BTreeMap::new(),
        };
        let output = capture_print_config(&view).await;
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
        };
        let info = ServerDisplayInfo::from_config(&srv);
        assert_eq!(info.api_key_source, "keyring");
        assert!(info.api_key.contains("bzr"));
        // The display shows the placeholder, not a real server name (since
        // ServerDisplayInfo doesn't know the server name — that's higher
        // up in ConfigView).
        assert!(info.api_key.contains("<server-name>"));
    }
}
