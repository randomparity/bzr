use colored::Colorize;
use serde::Serialize;

use super::{format_or_json, mask_api_key};
use crate::types::{OutputFormat, ServerExtensions, ServerVersion};

/// Combined server information for display.
pub struct ServerInfo<'a> {
    pub version: &'a ServerVersion,
    pub extensions: &'a ServerExtensions,
}

#[expect(clippy::print_stdout)]
pub fn print_server_info(info: &ServerInfo<'_>, format: OutputFormat) {
    #[derive(Serialize)]
    struct ServerInfoView<'a> {
        version: &'a str,
        extensions: &'a std::collections::HashMap<String, crate::types::ExtensionInfo>,
    }

    let view = ServerInfoView {
        version: &info.version.version,
        extensions: &info.extensions.extensions,
    };

    format_or_json(&view, format, |view| {
        println!("{} {}", "Bugzilla version:".bold(), view.version);
        if view.extensions.is_empty() {
            println!("\nNo extensions installed.");
        } else {
            println!("\n{}:", "Extensions".bold());
            for (name, info) in view.extensions {
                let ver = info.version.as_deref().unwrap_or("unknown");
                println!("  {name} ({ver})");
            }
        }
    });
}

#[derive(Serialize)]
struct ServerDisplayInfo {
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
struct ConfigView {
    config_file: String,
    default_server: Option<String>,
    servers: std::collections::BTreeMap<String, ServerDisplayInfo>,
}

#[expect(clippy::print_stdout)]
pub fn print_config(
    config: &crate::config::Config,
    config_path: &std::path::Path,
    format: OutputFormat,
) {
    let servers: std::collections::BTreeMap<String, ServerDisplayInfo> = config
        .servers
        .iter()
        .map(|(name, srv)| (name.clone(), ServerDisplayInfo::from_config(srv)))
        .collect();

    let view = ConfigView {
        config_file: config_path.to_string_lossy().into_owned(),
        default_server: config.default_server.clone(),
        servers,
    };

    format_or_json(&view, format, |v| {
        println!("Config file: {}\n", v.config_file);
        if let Some(ref def) = v.default_server {
            println!("Default server: {def}");
        }
        if v.servers.is_empty() {
            println!("No servers configured.");
        } else {
            for (name, s) in &v.servers {
                println!("\n[{name}]");
                println!("  URL:     {}", s.url);
                if let Some(ref email) = s.email {
                    println!("  Email:   {email}");
                }
                println!("  API Key: {}", s.api_key);
                println!("  Auth:    {}", s.auth_method);
            }
        }
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::useless_vec, clippy::single_char_pattern)]
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
