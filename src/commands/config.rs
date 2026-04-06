//! Configuration management commands.
//!
//! Config operations are pure local file I/O — no network client or auth
//! detection needed. The function is async for signature consistency with
//! sibling command modules.

use std::fmt::Write as _;

use crate::cli::ConfigAction;
use crate::config::{Config, ServerConfig};
use crate::error::Result;
use crate::output::{self, ConfigResult};
use crate::types::OutputFormat;

#[expect(
    clippy::unused_async,
    reason = "async for signature consistency with sibling execute fns"
)]
pub async fn execute(
    action: &ConfigAction,
    _server: Option<&str>,
    format: OutputFormat,
    _api: Option<crate::types::ApiMode>,
) -> Result<()> {
    match action {
        ConfigAction::SetServer {
            name,
            url,
            api_key,
            api_key_env,
            email,
            auth_method,
            tls_insecure,
        } => {
            if api_key.is_some() == api_key_env.is_some() {
                return Err(crate::error::BzrError::InputValidation(
                    "provide exactly one of --api-key or --api-key-env".into(),
                ));
            }
            let mut config = Config::load()?;
            let is_update = config.servers.contains_key(name.as_str());
            config.servers.insert(
                name.clone(),
                ServerConfig {
                    url: url.clone(),
                    api_key: api_key.clone(),
                    api_key_env: api_key_env.clone(),
                    api_key_keyring: None,
                    email: email.clone(),
                    auth_method: *auth_method,
                    api_mode: None,
                    server_version: None,
                    tls_insecure: *tls_insecure,
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
            if let Some(var_name) = api_key_env {
                let _ = write!(human, "\nAPI key source: env var {var_name}");
            } else {
                human.push_str("\nAPI key source: inline config value");
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
    use crate::test_helpers::capture_stdout;
    use crate::types::AuthMethod;

    async fn setup_config_env() -> (tokio::sync::MutexGuard<'static, ()>, tempfile::TempDir) {
        let lock = crate::ENV_LOCK.lock().await;
        let tmp = tempfile::TempDir::new().unwrap();
        let config_dir = tmp.path().join("bzr");
        std::fs::create_dir_all(&config_dir).unwrap();
        // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
        unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };
        (lock, tmp)
    }

    #[tokio::test]
    async fn set_default_on_empty_config_returns_error() {
        let (_lock, _tmp) = setup_config_env().await;
        let config = Config::default();
        config.save().unwrap();
        let result = execute(
            &ConfigAction::SetDefault {
                name: "nonexistent".into(),
            },
            None,
            OutputFormat::Table,
            None,
        )
        .await;
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), BzrError::Config(_)),
            "expected Config error for unknown server"
        );
    }

    #[tokio::test]
    async fn first_set_server_auto_sets_default() {
        let (_lock, _tmp) = setup_config_env().await;
        execute(
            &ConfigAction::SetServer {
                name: "first".into(),
                url: "https://first.example.com".into(),
                api_key: Some("first-key-1234567890".into()),
                api_key_env: None,
                email: None,
                auth_method: None,
                tls_insecure: false,
            },
            None,
            OutputFormat::Table,
            None,
        )
        .await
        .unwrap();
        let config = Config::load().unwrap();
        assert_eq!(config.default_server.as_deref(), Some("first"));
        assert!(config.servers.contains_key("first"));
    }

    #[tokio::test]
    async fn second_set_server_does_not_override_default() {
        let (_lock, _tmp) = setup_config_env().await;
        // Set up first server
        execute(
            &ConfigAction::SetServer {
                name: "first".into(),
                url: "https://first.example.com".into(),
                api_key: Some("first-key-1234567890".into()),
                api_key_env: None,
                email: None,
                auth_method: None,
                tls_insecure: false,
            },
            None,
            OutputFormat::Table,
            None,
        )
        .await
        .unwrap();
        // Add second server
        execute(
            &ConfigAction::SetServer {
                name: "second".into(),
                url: "https://second.example.com".into(),
                api_key: Some("second-key-1234567890".into()),
                api_key_env: None,
                email: None,
                auth_method: None,
                tls_insecure: false,
            },
            None,
            OutputFormat::Table,
            None,
        )
        .await
        .unwrap();
        let config = Config::load().unwrap();
        assert_eq!(
            config.default_server.as_deref(),
            Some("first"),
            "second server should not override existing default"
        );
        assert_eq!(config.servers.len(), 2);
    }

    #[tokio::test]
    async fn set_server_update_preserves_existing_default() {
        let (_lock, _tmp) = setup_config_env().await;
        for (name, url) in [
            ("first", "https://first.example.com"),
            ("second", "https://second.example.com"),
        ] {
            execute(
                &ConfigAction::SetServer {
                    name: name.into(),
                    url: url.into(),
                    api_key: Some(format!("{name}-key-1234567890")),
                    api_key_env: None,
                    email: None,
                    auth_method: None,
                    tls_insecure: false,
                },
                None,
                OutputFormat::Json,
                None,
            )
            .await
            .unwrap();
        }

        let (result, output) = capture_stdout(execute(
            &ConfigAction::SetServer {
                name: "second".into(),
                url: "https://updated.example.com".into(),
                api_key: Some("updated-key-1234567890".into()),
                api_key_env: None,
                email: Some("ops@example.com".into()),
                auth_method: Some(AuthMethod::QueryParam),
                tls_insecure: true,
            },
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_ok());

        let parsed = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["name"], "second");
        assert_eq!(parsed["action"], "updated");

        let config = Config::load().unwrap();
        assert_eq!(config.default_server.as_deref(), Some("first"));
        let server = &config.servers["second"];
        assert_eq!(server.url, "https://updated.example.com");
        assert_eq!(server.email.as_deref(), Some("ops@example.com"));
        assert_eq!(server.auth_method, Some(AuthMethod::QueryParam));
        assert!(server.tls_insecure);
    }

    #[tokio::test]
    async fn set_default_persists_selected_server() {
        let (_lock, _tmp) = setup_config_env().await;
        for (name, url) in [
            ("first", "https://first.example.com"),
            ("second", "https://second.example.com"),
        ] {
            execute(
                &ConfigAction::SetServer {
                    name: name.into(),
                    url: url.into(),
                    api_key: Some(format!("{name}-key-1234567890")),
                    api_key_env: None,
                    email: None,
                    auth_method: None,
                    tls_insecure: false,
                },
                None,
                OutputFormat::Json,
                None,
            )
            .await
            .unwrap();
        }

        let (result, output) = capture_stdout(execute(
            &ConfigAction::SetDefault {
                name: "second".into(),
            },
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_ok());

        let parsed = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["name"], "second");
        assert_eq!(parsed["action"], "updated");
        assert_eq!(
            Config::load().unwrap().default_server.as_deref(),
            Some("second")
        );
    }

    #[tokio::test]
    async fn show_json_includes_populated_server_details() {
        let (_lock, _tmp) = setup_config_env().await;
        execute(
            &ConfigAction::SetServer {
                name: "prod".into(),
                url: "https://prod.example.com".into(),
                api_key: Some("abcdef1234567890".into()),
                api_key_env: None,
                email: Some("admin@example.com".into()),
                auth_method: Some(AuthMethod::Header),
                tls_insecure: true,
            },
            None,
            OutputFormat::Json,
            None,
        )
        .await
        .unwrap();

        let (result, output) =
            capture_stdout(execute(&ConfigAction::Show, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());

        let parsed = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed["default_server"], "prod");
        assert_eq!(parsed["servers"]["prod"]["url"], "https://prod.example.com");
        assert_eq!(parsed["servers"]["prod"]["email"], "admin@example.com");
        assert_eq!(parsed["servers"]["prod"]["auth_method"], "header");
        assert_eq!(parsed["servers"]["prod"]["tls_insecure"], true);
        assert_eq!(parsed["servers"]["prod"]["api_key"], "abcdef12...");
        assert_eq!(parsed["servers"]["prod"]["api_key_source"], "inline");
    }

    #[tokio::test]
    async fn set_server_with_env_var_persists_env_source() {
        let (_lock, _tmp) = setup_config_env().await;
        execute(
            &ConfigAction::SetServer {
                name: "prod".into(),
                url: "https://prod.example.com".into(),
                api_key: None,
                api_key_env: Some("BZR_API_KEY".into()),
                email: None,
                auth_method: None,
                tls_insecure: false,
            },
            None,
            OutputFormat::Json,
            None,
        )
        .await
        .unwrap();

        let config = Config::load().unwrap();
        let server = &config.servers["prod"];
        assert_eq!(server.api_key, None);
        assert_eq!(server.api_key_env.as_deref(), Some("BZR_API_KEY"));
    }
}
