use std::fmt::Write;

use crate::cli::ConfigAction;
use crate::config::{Config, ServerConfig};
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub fn execute(action: &ConfigAction, format: OutputFormat) -> Result<()> {
    match action {
        ConfigAction::SetServer {
            name,
            url,
            api_key,
            email,
            auth_method,
        } => {
            let mut config = Config::load()?;
            config.servers.insert(
                name.clone(),
                ServerConfig {
                    url: url.clone(),
                    api_key: api_key.clone(),
                    email: email.clone(),
                    auth_method: *auth_method,
                    api_mode: None,
                    server_version: None,
                },
            );
            if config.default_server.is_none() {
                config.default_server = Some(name.clone());
            }
            let is_default = config.default_server.as_deref() == Some(name.as_str());
            let path = Config::path()?;
            config.save()?;

            let mut human = format!("Server '{name}' configured at {url}");
            if is_default {
                human.push_str("\nSet as default server.");
            }
            let _ = write!(human, "\nConfig file: {}", path.display());

            output::print_result(
                &serde_json::json!({
                    "name": name,
                    "url": url,
                    "is_default": is_default,
                    "config_file": path.to_string_lossy(),
                    "resource": "server_config",
                    "action": "configured",
                }),
                &human,
                format,
            );
        }
        ConfigAction::SetDefault { name } => {
            let mut config = Config::load()?;
            if !config.servers.contains_key(name) {
                return Err(crate::error::BzrError::config(format!(
                    "server '{name}' not found"
                )));
            }
            config.default_server = Some(name.clone());
            let path = Config::path()?;
            config.save()?;

            output::print_result(
                &serde_json::json!({
                    "name": name,
                    "config_file": path.to_string_lossy(),
                    "resource": "default_server",
                    "action": "updated",
                }),
                &format!(
                    "Default server set to '{name}'\nConfig file: {}",
                    path.display()
                ),
                format,
            );
        }
        ConfigAction::Show => {
            show_config(format)?;
        }
    }
    Ok(())
}

#[expect(clippy::print_stdout)]
fn show_config(format: OutputFormat) -> Result<()> {
    let config = Config::load()?;
    let path = Config::path()?;

    match format {
        OutputFormat::Json => {
            let mut servers = serde_json::Map::new();
            for (name, srv) in &config.servers {
                let masked_key = mask_api_key(&srv.api_key);
                servers.insert(
                    name.clone(),
                    serde_json::json!({
                        "url": srv.url,
                        "email": srv.email,
                        "api_key": masked_key,
                        "auth_method": srv.auth_method
                            .as_ref()
                            .map(ToString::to_string),
                    }),
                );
            }
            let out = serde_json::json!({
                "config_file": path.to_string_lossy(),
                "default_server": config.default_server,
                "servers": servers,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&out).unwrap_or_else(|_| "{}".into())
            );
        }
        OutputFormat::Table => {
            println!("Config file: {}\n", path.display());
            if let Some(ref def) = config.default_server {
                println!("Default server: {def}");
            }
            if config.servers.is_empty() {
                println!("No servers configured.");
            } else {
                for (name, srv) in &config.servers {
                    let masked_key = mask_api_key(&srv.api_key);
                    let auth_display = match &srv.auth_method {
                        Some(m) => m.to_string(),
                        None => "auto (not yet detected)".into(),
                    };
                    println!("\n[{name}]");
                    println!("  URL:     {}", srv.url);
                    if let Some(ref email) = srv.email {
                        println!("  Email:   {email}");
                    }
                    println!("  API Key: {masked_key}");
                    println!("  Auth:    {auth_display}");
                }
            }
        }
    }
    Ok(())
}

fn mask_api_key(key: &str) -> String {
    if key.len() > 8 {
        format!("{}...", &key[..8])
    } else {
        "***".into()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::error::BzrError;

    #[test]
    fn mask_api_key_long_key_shows_prefix() {
        assert_eq!(mask_api_key("abcdefghijklmnop"), "abcdefgh...");
    }

    #[test]
    fn mask_api_key_short_key_fully_masked() {
        assert_eq!(mask_api_key("short"), "***");
    }

    #[test]
    fn mask_api_key_exactly_8_chars_fully_masked() {
        assert_eq!(mask_api_key("12345678"), "***");
    }

    #[test]
    fn mask_api_key_empty_string_fully_masked() {
        assert_eq!(mask_api_key(""), "***");
    }

    /// Combined test for config operations that require env::set_var.
    /// Grouped in a single test to avoid env var race conditions with
    /// parallel test execution.
    #[test]
    fn config_operations_with_file_io() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", tmp.path());

        // 1. set-default on empty config returns error
        let config = Config::default();
        config.save().unwrap();
        let result = execute(
            &ConfigAction::SetDefault {
                name: "nonexistent".into(),
            },
            OutputFormat::Table,
        );
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), BzrError::Config(_)),
            "expected Config error for unknown server"
        );

        // 2. First set-server auto-sets default
        execute(
            &ConfigAction::SetServer {
                name: "first".into(),
                url: "https://first.example.com".into(),
                api_key: "first-key-1234567890".into(),
                email: None,
                auth_method: None,
            },
            OutputFormat::Table,
        )
        .unwrap();
        let config = Config::load().unwrap();
        assert_eq!(config.default_server.as_deref(), Some("first"));
        assert!(config.servers.contains_key("first"));

        // 3. Second set-server does not override default
        execute(
            &ConfigAction::SetServer {
                name: "second".into(),
                url: "https://second.example.com".into(),
                api_key: "second-key-1234567890".into(),
                email: None,
                auth_method: None,
            },
            OutputFormat::Table,
        )
        .unwrap();
        let config = Config::load().unwrap();
        assert_eq!(
            config.default_server.as_deref(),
            Some("first"),
            "second server should not override existing default"
        );
        assert_eq!(config.servers.len(), 2);
    }
}
