//! Configuration management commands.
//!
//! Unlike other command modules, `execute()` is **synchronous** because config
//! operations are pure local file I/O — no network client or auth detection needed.

use std::fmt::Write as _;

use crate::cli::ConfigAction;
use crate::config::{Config, ServerConfig};
use crate::error::Result;
use crate::output::{self, ConfigResult};
use crate::types::OutputFormat;

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
            let is_update = config.servers.contains_key(name.as_str());
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

            let verb = if is_update { "updated" } else { "configured" };
            let mut human = format!("Server '{name}' {verb} at {url}");
            if is_default {
                human.push_str("\nSet as default server.");
            }
            let _ = write!(human, "\nConfig file: {}", path.display());

            output::print_result(
                &ConfigResult::configured(
                    name.as_str(),
                    url.as_str(),
                    is_default,
                    path.to_string_lossy(),
                    is_update,
                ),
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
                &ConfigResult::default_set(name.as_str(), path.to_string_lossy()),
                &format!(
                    "Default server set to '{name}'\nConfig file: {}",
                    path.display()
                ),
                format,
            );
        }
        ConfigAction::Show => {
            let config = Config::load()?;
            let path = Config::path()?;
            let view = output::ConfigView::from_config(&config, &path);
            output::print_config(&view, format);
        }
    }
    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::error::BzrError;

    /// Combined test for config operations that require `env::set_var`.
    /// Grouped in a single test to avoid env var race conditions with
    /// parallel test execution.
    #[test]
    fn config_operations_with_file_io() {
        let _lock = crate::ENV_LOCK.blocking_lock();
        let tmp = tempfile::TempDir::new().unwrap();
        // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };

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
