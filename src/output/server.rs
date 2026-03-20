use colored::Colorize;
use serde::Serialize;

use super::common::{mask_api_key, print_colored_field, print_formatted, print_optional_field};
use crate::types::{OutputFormat, ServerInfoResponse};

/// Combined server information for display.
#[derive(Serialize)]
#[non_exhaustive]
pub struct ServerInfo<'a> {
    pub version: &'a str,
    pub extensions: &'a std::collections::HashMap<String, crate::types::ExtensionInfo>,
}

impl<'a> From<&'a ServerInfoResponse> for ServerInfo<'a> {
    fn from(info: &'a ServerInfoResponse) -> Self {
        Self {
            version: &info.version.version,
            extensions: &info.extensions.extensions,
        }
    }
}

#[expect(clippy::print_stdout)]
pub fn print_server_info(info: &ServerInfo<'_>, format: OutputFormat) {
    print_formatted(info, format, |info| {
        println!("{} {}", "Bugzilla version:".bold(), info.version);
        if info.extensions.is_empty() {
            println!("\nNo extensions installed.");
        } else {
            println!("\n{}:", "Extensions".bold());
            for (name, ext) in info.extensions {
                let ver = ext.version.as_deref().unwrap_or("unknown");
                println!("  {name} ({ver})");
            }
        }
    });
}

#[derive(Serialize)]
pub struct ServerDisplayInfo {
    url: String,
    email: Option<String>,
    api_key: String,
    auth_method: String,
}

impl ServerDisplayInfo {
    fn from_config(srv: &crate::config::ServerConfig) -> Self {
        Self {
            url: srv.url.clone(),
            email: srv.email.clone(),
            api_key: mask_api_key(&srv.api_key),
            auth_method: srv
                .auth_method
                .as_ref()
                .map_or_else(|| "auto (not yet detected)".into(), ToString::to_string),
        }
    }
}

#[derive(Serialize)]
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

#[expect(clippy::print_stdout)]
pub fn print_config(view: &ConfigView, format: OutputFormat) {
    print_formatted(view, format, |v| {
        println!("Config file: {}\n", v.config_file);
        if let Some(ref def) = v.default_server {
            println!("Default server: {def}");
        }
        if v.servers.is_empty() {
            println!("No servers configured.");
        } else {
            for (name, s) in &v.servers {
                println!("\n[{name}]");
                print_colored_field("URL", &s.url);
                print_optional_field("Email", s.email.as_deref());
                print_colored_field("API Key", &s.api_key);
                print_colored_field("Auth", &s.auth_method);
            }
        }
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use crate::types::{ServerExtensions, ServerVersion};

    #[test]
    fn print_server_info_json_combined() {
        let version = ServerVersion {
            version: "5.0.4".into(),
        };
        let extensions = ServerExtensions {
            extensions: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "BmpConvert".into(),
                    crate::types::ExtensionInfo {
                        version: Some("1.0".into()),
                    },
                );
                m
            },
        };
        let combined = serde_json::json!({
            "version": version.version,
            "extensions": extensions.extensions,
        });
        let json = serde_json::to_string_pretty(&combined).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["version"], "5.0.4");
        assert!(parsed["extensions"]["BmpConvert"].is_object());
    }
}
