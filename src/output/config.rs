use serde::Serialize;

use super::common::{mask_api_key, print_field, print_formatted, print_optional_field};
use crate::types::{AuthMethod, OutputFormat};

#[derive(Serialize)]
#[non_exhaustive]
pub struct ServerDisplayInfo {
    url: String,
    email: Option<String>,
    api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    auth_method: Option<AuthMethod>,
}

impl ServerDisplayInfo {
    fn from_config(srv: &crate::config::ServerConfig) -> Self {
        Self {
            url: srv.url.clone(),
            email: srv.email.clone(),
            api_key: mask_api_key(&srv.api_key),
            auth_method: srv.auth_method,
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
                print_field("URL", &s.url);
                print_optional_field("Email", s.email.as_deref());
                print_field("API Key", &s.api_key);
                let auth_display = s.auth_method.as_ref().map_or_else(
                    || "auto (not yet detected)".to_string(),
                    ToString::to_string,
                );
                print_field("Auth", &auth_display);
            }
        }
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

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
}
