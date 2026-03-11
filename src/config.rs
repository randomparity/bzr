use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{BzrError, Result};

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub default_server: Option<String>,
    #[serde(default)]
    pub servers: HashMap<String, ServerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub url: String,
    pub api_key: String,
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| BzrError::config("cannot determine config directory"))?;
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

    pub fn active_server(&self, server_name: Option<&str>) -> Result<&ServerConfig> {
        let name = server_name
            .or(self.default_server.as_deref())
            .ok_or_else(|| {
                BzrError::config(
                    "no server configured. Run `bzr config set-server <name> --url <url> --api-key <key>` first",
                )
            })?;
        self.servers
            .get(name)
            .ok_or_else(|| BzrError::config(format!("server '{}' not found in config", name)))
    }
}
