use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{BzrError, Result};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub default_server: Option<String>,
    #[serde(default)]
    pub servers: HashMap<String, ServerConfig>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    Header,
    QueryParam,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiMode {
    Rest,
    #[serde(rename = "xmlrpc")]
    XmlRpc,
    Hybrid,
}

impl fmt::Display for ApiMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiMode::Rest => write!(f, "rest"),
            ApiMode::XmlRpc => write!(f, "xmlrpc"),
            ApiMode::Hybrid => write!(f, "hybrid"),
        }
    }
}

impl FromStr for ApiMode {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "rest" => Ok(ApiMode::Rest),
            "xmlrpc" => Ok(ApiMode::XmlRpc),
            "hybrid" => Ok(ApiMode::Hybrid),
            _ => Err(format!(
                "invalid API mode '{s}': expected 'rest', 'xmlrpc', or 'hybrid'"
            )),
        }
    }
}

impl FromStr for AuthMethod {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "header" => Ok(AuthMethod::Header),
            "query_param" => Ok(AuthMethod::QueryParam),
            _ => Err(format!(
                "invalid auth method '{s}': expected 'header' or 'query_param'"
            )),
        }
    }
}

impl fmt::Display for AuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthMethod::Header => write!(f, "header"),
            AuthMethod::QueryParam => write!(f, "query_param"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub url: String,
    pub api_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_method: Option<AuthMethod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_mode: Option<ApiMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_version: Option<String>,
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let config_dir = if let Ok(val) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(val)
        } else {
            let home = dirs::home_dir()
                .ok_or_else(|| BzrError::config("cannot determine home directory"))?;
            home.join(".config")
        };
        Ok(config_dir.join("bzr").join("config.toml"))
    }

    pub fn load() -> Result<Config> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Config::default());
        }
        let content = fs::read_to_string(&path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn active_server_named<'a>(
        &'a self,
        server_name: Option<&'a str>,
    ) -> Result<(&'a str, &'a ServerConfig)> {
        let name = self.resolve_server_name(server_name)?;
        let srv = self
            .servers
            .get(name)
            .ok_or_else(|| BzrError::config(format!("server '{name}' not found in config")))?;
        Ok((name, srv))
    }

    pub fn resolve_server_name<'a>(&'a self, server_name: Option<&'a str>) -> Result<&'a str> {
        server_name
            .or(self.default_server.as_deref())
            .ok_or_else(|| {
                BzrError::config(
                    "no server configured. Run `bzr config set-server <name> --url <url> --api-key <key>` first",
                )
            })
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::env;

    fn make_server_config(url: &str) -> ServerConfig {
        ServerConfig {
            url: url.to_string(),
            api_key: "test-key".to_string(),
            email: None,
            auth_method: None,
            api_mode: None,
            server_version: None,
        }
    }

    fn make_config_with_server() -> Config {
        let mut servers = HashMap::new();
        servers.insert(
            "myserver".to_string(),
            make_server_config("https://bugzilla.example.com"),
        );
        Config {
            default_server: Some("myserver".to_string()),
            servers,
        }
    }

    /// Combined test for operations that require `env::set_var`.
    /// Grouped in a single test to avoid env var race conditions with
    /// parallel test execution.
    #[test]
    fn config_file_io_operations() {
        let _lock = crate::ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        env::set_var("XDG_CONFIG_HOME", tmp.path());

        // 1. Load returns default when no file exists
        let config = Config::load().unwrap();
        assert!(config.default_server.is_none());
        assert!(config.servers.is_empty());

        // 2. Save and load roundtrip
        let original = make_config_with_server();
        original.save().unwrap();

        let loaded = Config::load().unwrap();
        assert_eq!(loaded.default_server.as_deref(), Some("myserver"));
        assert!(loaded.servers.contains_key("myserver"));
        let srv = loaded.servers.get("myserver").unwrap();
        assert_eq!(srv.url, "https://bugzilla.example.com");
        assert_eq!(srv.api_key, "test-key");
    }

    #[test]
    fn resolve_server_name_returns_default_when_none() {
        let config = make_config_with_server();
        let name = config.resolve_server_name(None).unwrap();
        assert_eq!(name, "myserver");
    }

    #[test]
    fn resolve_server_name_returns_explicit_name() {
        let config = make_config_with_server();
        let name = config.resolve_server_name(Some("other")).unwrap();
        assert_eq!(name, "other");
    }

    #[test]
    fn resolve_server_name_errors_when_no_default_and_no_name() {
        let config = Config::default();
        let result = config.resolve_server_name(None);
        assert!(result.is_err());
    }

    #[test]
    fn active_server_named_returns_correct_server() {
        let config = make_config_with_server();
        let (name, srv) = config.active_server_named(Some("myserver")).unwrap();
        assert_eq!(name, "myserver");
        assert_eq!(srv.url, "https://bugzilla.example.com");
    }

    #[test]
    fn active_server_named_errors_for_unknown_server() {
        let config = make_config_with_server();
        let result = config.active_server_named(Some("nonexistent"));
        assert!(result.is_err());
    }

    #[test]
    fn api_mode_from_str_valid() {
        assert_eq!(ApiMode::from_str("rest").unwrap(), ApiMode::Rest);
        assert_eq!(ApiMode::from_str("xmlrpc").unwrap(), ApiMode::XmlRpc);
        assert_eq!(ApiMode::from_str("hybrid").unwrap(), ApiMode::Hybrid);
    }

    #[test]
    fn api_mode_from_str_invalid() {
        let result = ApiMode::from_str("invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid API mode"));
    }

    #[test]
    fn auth_method_from_str_valid() {
        assert_eq!(AuthMethod::from_str("header").unwrap(), AuthMethod::Header);
        assert_eq!(
            AuthMethod::from_str("query_param").unwrap(),
            AuthMethod::QueryParam
        );
    }

    #[test]
    fn auth_method_from_str_invalid() {
        let result = AuthMethod::from_str("invalid");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid auth method"));
    }

    #[test]
    fn api_mode_display_formatting() {
        assert_eq!(ApiMode::Rest.to_string(), "rest");
        assert_eq!(ApiMode::XmlRpc.to_string(), "xmlrpc");
        assert_eq!(ApiMode::Hybrid.to_string(), "hybrid");
    }
}
