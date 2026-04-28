use std::collections::HashMap;
use std::fs;
use std::io::Write as _;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{BzrError, Result};
use crate::types::{ApiMode, AuthMethod, BugTemplate, SavedQuery};

#[derive(Debug, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct Config {
    pub default_server: Option<String>,
    #[serde(default)]
    pub servers: HashMap<String, ServerConfig>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub templates: HashMap<String, BugTemplate>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub queries: HashMap<String, SavedQuery>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ServerConfig {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_keyring: Option<KeyringRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_method: Option<AuthMethod>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_mode: Option<ApiMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_version: Option<String>,
    /// Accept invalid TLS certificates (self-signed, expired, etc.).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tls_insecure: bool,
    /// Path to a PEM-encoded CA certificate for this server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_ca_cert: Option<PathBuf>,
    /// SHA-256 fingerprint of the pinned server certificate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin_sha256: Option<String>,
    /// Issuer DN stored alongside the pin for rotation detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin_issuer: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub struct KeyringRef {
    /// Keyring service name. Defaults to "bzr" when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    /// Account/username within the service. Defaults to the server name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

impl KeyringRef {
    pub fn service_or_default(&self) -> &str {
        self.service.as_deref().unwrap_or("bzr")
    }

    pub fn account_or_default<'a>(&'a self, server_name: &'a str) -> &'a str {
        self.account.as_deref().unwrap_or(server_name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialSourceKind {
    Inline,
    Env,
    Keyring,
}

#[derive(Debug)]
pub enum CredentialSource<'a> {
    Inline(&'a str),
    EnvVar(&'a str),
    Keyring { service: &'a str, account: &'a str },
}

impl CredentialSource<'_> {
    pub fn kind(&self) -> CredentialSourceKind {
        match self {
            CredentialSource::Inline(_) => CredentialSourceKind::Inline,
            CredentialSource::EnvVar(_) => CredentialSourceKind::Env,
            CredentialSource::Keyring { .. } => CredentialSourceKind::Keyring,
        }
    }
}

impl CredentialSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            CredentialSourceKind::Inline => "inline",
            CredentialSourceKind::Env => "env",
            CredentialSourceKind::Keyring => "keyring",
        }
    }
}

impl ServerConfig {
    pub fn validate(&self, server_name: &str) -> Result<()> {
        self.credential_source()
            .map(|_| ())
            .map_err(|err| BzrError::config(format!("server '{server_name}': {err}")))?;
        self.validate_tls(server_name)
    }

    pub fn credential_source(&self) -> Result<CredentialSource<'_>> {
        let count = usize::from(self.api_key.is_some())
            + usize::from(self.api_key_env.is_some())
            + usize::from(self.api_key_keyring.is_some());
        match count {
            0 => Err(BzrError::config(
                "server config must define one of 'api_key', 'api_key_env', or 'api_key_keyring'",
            )),
            1 => {
                if let Some(api_key) = self.api_key.as_deref() {
                    Ok(CredentialSource::Inline(api_key))
                } else if let Some(var_name) = self.api_key_env.as_deref() {
                    Ok(CredentialSource::EnvVar(var_name))
                } else {
                    let r = self.api_key_keyring.as_ref().ok_or_else(|| {
                        BzrError::config("internal: keyring credential unexpectedly missing")
                    })?;
                    // Empty string means "default to the server_name"; the
                    // real account is resolved in resolve_api_key() which
                    // has the server name in scope. We cannot use
                    // KeyringRef::account_or_default here because that would
                    // require plumbing the server name through every caller.
                    Ok(CredentialSource::Keyring {
                        service: r.service_or_default(),
                        account: r.account.as_deref().unwrap_or(""),
                    })
                }
            }
            _ => Err(BzrError::config(
                "server config cannot define multiple API key sources \
                 (api_key, api_key_env, api_key_keyring)",
            )),
        }
    }

    pub fn credential_source_kind(&self) -> Result<CredentialSourceKind> {
        Ok(self.credential_source()?.kind())
    }

    pub fn resolve_api_key(&self, server_name: &str) -> Result<String> {
        match self.credential_source()? {
            CredentialSource::Inline(api_key) => Ok(api_key.to_string()),
            CredentialSource::EnvVar(var_name) => {
                let value = std::env::var(var_name).map_err(|_| {
                    BzrError::config(format!(
                        "server '{server_name}' uses API key env var '{var_name}', but it is not set"
                    ))
                })?;
                if value.is_empty() {
                    return Err(BzrError::config(format!(
                        "server '{server_name}' uses API key env var '{var_name}', but it is empty"
                    )));
                }
                Ok(value)
            }
            CredentialSource::Keyring { service, account } => {
                // Empty `account` means "default to server_name" (see the
                // sentinel explanation in credential_source()).
                let account = if account.is_empty() {
                    server_name
                } else {
                    account
                };
                crate::credentials::keyring::retrieve(service, account)
            }
        }
    }

    pub fn validate_tls(&self, server_name: &str) -> Result<()> {
        let ctx = |msg: &str| BzrError::config(format!("server '{server_name}': {msg}"));

        if self.tls_insecure && self.tls_ca_cert.is_some() {
            return Err(ctx("tls_insecure and tls_ca_cert are mutually exclusive"));
        }
        if self.tls_insecure && self.tls_pin_sha256.is_some() {
            return Err(ctx(
                "tls_insecure and tls_pin_sha256 are mutually exclusive",
            ));
        }
        if self.tls_ca_cert.is_some() && self.tls_pin_sha256.is_some() {
            return Err(ctx("tls_ca_cert and tls_pin_sha256 are mutually exclusive"));
        }
        if let Some(pin) = &self.tls_pin_sha256 {
            crate::tls::fingerprint::parse_pin(pin)
                .map_err(|e| ctx(&format!("invalid tls_pin_sha256: {e}")))?;
        }
        if let Some(ca_cert) = &self.tls_ca_cert {
            if !ca_cert.exists() {
                return Err(ctx(&format!(
                    "tls_ca_cert '{}' does not exist",
                    ca_cert.display()
                )));
            }
        }
        Ok(())
    }
}

impl Config {
    pub fn path() -> Result<PathBuf> {
        let config_dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(dirs::config_dir)
            .ok_or_else(|| BzrError::config("cannot determine config directory"))?;
        Ok(config_dir.join("bzr").join("config.toml"))
    }

    pub fn load() -> Result<Config> {
        let path = Self::path()?;
        match fs::read_to_string(&path) {
            Ok(content) => {
                Self::warn_on_insecure_permissions(&path);
                let config: Config = toml::from_str(&content)?;
                config.validate()?;
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save(&self) -> Result<()> {
        self.validate()?;
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            let parent_exists = parent.exists();
            fs::create_dir_all(parent)?;
            if !parent_exists {
                set_private_directory_permissions(parent)?;
            }
        }
        let content = toml::to_string_pretty(self)?;
        let file_exists = path.exists();
        write_private_file(&path, &content)?;
        if !file_exists {
            set_private_file_permissions(&path)?;
        }
        Self::warn_on_insecure_permissions(&path);
        Ok(())
    }

    pub fn resolve_server<'a>(
        &'a self,
        server_name: Option<&'a str>,
    ) -> Result<(&'a str, &'a ServerConfig)> {
        let name = self.resolve_server_name_only(server_name)?;
        let srv = self
            .servers
            .get(name)
            .ok_or_else(|| BzrError::config(format!("server '{name}' not found in config")))?;
        Ok((name, srv))
    }

    pub fn resolve_server_name_only<'a>(&'a self, server_name: Option<&'a str>) -> Result<&'a str> {
        server_name
            .or(self.default_server.as_deref())
            .ok_or_else(|| {
                BzrError::config(
                    "no server configured. Run `bzr config set-server <name> --url <url> --api-key-env <env-var>` first",
                )
            })
    }

    fn warn_on_insecure_permissions(path: &std::path::Path) {
        #[cfg(unix)]
        {
            if let Some(parent) = path.parent() {
                warn_if_path_permissions_too_open(parent, 0o077, "config directory");
            }
            if path.exists() {
                warn_if_path_permissions_too_open(path, 0o077, "config file");
            }
        }
    }

    fn validate(&self) -> Result<()> {
        for (name, server) in &self.servers {
            server.validate(name)?;
        }
        Ok(())
    }
}

#[cfg(unix)]
fn write_private_file(path: &std::path::Path, content: &str) -> Result<()> {
    use std::fs::OpenOptions;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(content.as_bytes())?;
    Ok(())
}

#[cfg(not(unix))]
fn write_private_file(path: &std::path::Path, content: &str) -> Result<()> {
    fs::write(path, content)?;
    Ok(())
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_file_permissions(_path: &std::path::Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn warn_if_path_permissions_too_open(path: &std::path::Path, mask: u32, kind: &str) {
    use std::os::unix::fs::PermissionsExt;

    let Ok(metadata) = fs::metadata(path) else {
        return;
    };
    let mode = metadata.permissions().mode();
    if mode & mask == 0 {
        return;
    }

    warn_security(&format!(
        "{kind} '{}' has overly broad permissions ({:o}); expected owner-only access. Fix with `chmod {}` '{}'",
        path.display(),
        mode & 0o777,
        if kind == "config directory" { "700" } else { "600" },
        path.display()
    ));
}

#[expect(clippy::print_stderr)]
fn warn_security(message: &str) {
    eprintln!("warning: {message}");
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use std::env;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::str::FromStr;

    fn make_server_config(url: &str) -> ServerConfig {
        ServerConfig {
            url: url.to_string(),
            api_key: Some("test-key".to_string()),
            api_key_env: None,
            api_key_keyring: None,
            email: None,
            auth_method: None,
            api_mode: None,
            server_version: None,
            tls_insecure: false,
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_issuer: None,
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
            templates: HashMap::new(),
            queries: HashMap::new(),
        }
    }

    /// Combined test for operations that require `env::set_var`.
    /// Grouped in a single test to avoid env var race conditions with
    /// parallel test execution.
    #[test]
    fn config_file_io_operations() {
        let _lock = crate::ENV_LOCK.blocking_lock();
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
        unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

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
        assert_eq!(srv.api_key.as_deref(), Some("test-key"));
    }

    #[test]
    fn resolve_server_name_only_returns_default_when_none() {
        let config = make_config_with_server();
        let name = config.resolve_server_name_only(None).unwrap();
        assert_eq!(name, "myserver");
    }

    #[test]
    fn resolve_server_name_only_returns_explicit_name() {
        let config = make_config_with_server();
        let name = config.resolve_server_name_only(Some("other")).unwrap();
        assert_eq!(name, "other");
    }

    #[test]
    fn resolve_server_name_only_errors_when_no_default_and_no_name() {
        let config = Config::default();
        let result = config.resolve_server_name_only(None);
        assert!(result.is_err());
    }

    #[test]
    fn resolve_server_returns_correct_server() {
        let config = make_config_with_server();
        let (name, srv) = config.resolve_server(Some("myserver")).unwrap();
        assert_eq!(name, "myserver");
        assert_eq!(srv.url, "https://bugzilla.example.com");
    }

    #[test]
    fn resolve_server_errors_for_unknown_server() {
        let config = make_config_with_server();
        let result = config.resolve_server(Some("nonexistent"));
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

    #[test]
    fn config_roundtrips_saved_queries() {
        let _lock = crate::ENV_LOCK.blocking_lock();
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
        unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

        let mut config = make_config_with_server();
        let query = crate::types::SavedQuery {
            kind: crate::types::QueryKind::List,
            product: vec!["Firefox".into()],
            status: vec!["NEW".into()],
            limit: Some(25),
            ..Default::default()
        };
        config.queries.insert("firefox-new".into(), query);
        config.save().unwrap();

        let loaded = Config::load().unwrap();
        assert!(loaded.queries.contains_key("firefox-new"));
        let q = loaded.queries.get("firefox-new").unwrap();
        assert_eq!(q.product, vec!["Firefox"]);
        assert_eq!(q.status, vec!["NEW"]);
        assert_eq!(q.limit, Some(25));
    }

    #[test]
    fn config_empty_queries_not_serialized() {
        let config = make_config_with_server();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(
            !toml_str.contains("[queries"),
            "empty queries should not appear in TOML"
        );
    }

    #[test]
    fn config_load_rejects_multiple_api_key_sources() {
        let _lock = crate::ENV_LOCK.blocking_lock();
        let tmp = tempfile::tempdir().unwrap();
        let config_dir = tmp.path().join("bzr");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            config_dir.join("config.toml"),
            r#"
default_server = "test"

[servers.test]
url = "https://bugzilla.example.com"
api_key = "inline"
api_key_env = "BZR_TEST_API_KEY"
"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            fs::set_permissions(&config_dir, fs::Permissions::from_mode(0o700)).unwrap();
            fs::set_permissions(
                config_dir.join("config.toml"),
                fs::Permissions::from_mode(0o600),
            )
            .unwrap();
        }
        // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
        unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

        let err = Config::load().unwrap_err();
        assert!(err.to_string().contains("server 'test'"));
        assert!(err.to_string().contains("multiple API key sources"));
    }

    #[test]
    fn env_backed_server_resolves_api_key_from_environment() {
        let _lock = crate::ENV_LOCK.blocking_lock();
        // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
        unsafe { env::set_var("BZR_TEST_API_KEY", "secret-from-env") };

        let server = ServerConfig {
            url: "https://bugzilla.example.com".into(),
            api_key: None,
            api_key_env: Some("BZR_TEST_API_KEY".into()),
            api_key_keyring: None,
            email: None,
            auth_method: None,
            api_mode: None,
            server_version: None,
            tls_insecure: false,
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_issuer: None,
        };

        assert_eq!(server.resolve_api_key("test").unwrap(), "secret-from-env");
        assert_eq!(
            server.credential_source_kind().unwrap(),
            CredentialSourceKind::Env
        );
    }

    #[test]
    fn server_config_rejects_multiple_api_key_sources() {
        let server = ServerConfig {
            url: "https://bugzilla.example.com".into(),
            api_key: Some("inline".into()),
            api_key_env: Some("BZR_TEST_API_KEY".into()),
            api_key_keyring: None,
            email: None,
            auth_method: None,
            api_mode: None,
            server_version: None,
            tls_insecure: false,
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_issuer: None,
        };

        let err = server.credential_source().unwrap_err();
        assert!(err.to_string().contains("multiple API key sources"));
    }

    #[test]
    fn keyring_ref_defaults() {
        let r = KeyringRef {
            service: None,
            account: None,
        };
        assert_eq!(r.service_or_default(), "bzr");
        assert_eq!(r.account_or_default("prod"), "prod");
    }

    #[test]
    fn keyring_ref_explicit() {
        let r = KeyringRef {
            service: Some("custom".into()),
            account: Some("acct".into()),
        };
        assert_eq!(r.service_or_default(), "custom");
        assert_eq!(r.account_or_default("prod"), "acct");
    }

    #[test]
    fn keyring_ref_toml_roundtrip_empty() {
        let toml_str = r#"
url = "https://example.com"
api_key_keyring = {}
"#;
        let srv: ServerConfig = toml::from_str(toml_str).unwrap();
        assert!(srv.api_key_keyring.is_some());
        let r = srv.api_key_keyring.as_ref().unwrap();
        assert!(r.service.is_none());
        assert!(r.account.is_none());
    }

    #[test]
    fn keyring_ref_toml_roundtrip_full() {
        let toml_str = r#"
url = "https://example.com"
api_key_keyring = { service = "bzr", account = "dave" }
"#;
        let srv: ServerConfig = toml::from_str(toml_str).unwrap();
        let r = srv.api_key_keyring.as_ref().unwrap();
        assert_eq!(r.service.as_deref(), Some("bzr"));
        assert_eq!(r.account.as_deref(), Some("dave"));
    }

    #[test]
    fn credential_source_keyring_variant() {
        let server = ServerConfig {
            url: "https://example.com".into(),
            api_key: None,
            api_key_env: None,
            api_key_keyring: Some(KeyringRef {
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
        };
        match server.credential_source().unwrap() {
            CredentialSource::Keyring { service, account } => {
                assert_eq!(service, "bzr");
                // account defaults handled at resolve time via server_name
                assert_eq!(account, "");
            }
            other => panic!("expected Keyring variant, got {other:?}"),
        }
        assert_eq!(
            server.credential_source_kind().unwrap(),
            CredentialSourceKind::Keyring
        );
    }

    #[test]
    fn credential_source_rejects_keyring_with_inline() {
        let server = ServerConfig {
            url: "https://example.com".into(),
            api_key: Some("k".into()),
            api_key_env: None,
            api_key_keyring: Some(KeyringRef {
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
        };
        let err = server.credential_source().unwrap_err();
        assert!(err.to_string().contains("multiple API key sources"));
    }

    #[test]
    fn credential_source_rejects_keyring_with_env() {
        let server = ServerConfig {
            url: "https://example.com".into(),
            api_key: None,
            api_key_env: Some("VAR".into()),
            api_key_keyring: Some(KeyringRef {
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
        };
        let err = server.credential_source().unwrap_err();
        assert!(err.to_string().contains("multiple API key sources"));
    }

    #[test]
    fn credential_source_rejects_all_three() {
        let server = ServerConfig {
            url: "https://example.com".into(),
            api_key: Some("k".into()),
            api_key_env: Some("VAR".into()),
            api_key_keyring: Some(KeyringRef {
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
        };
        let err = server.credential_source().unwrap_err();
        assert!(err.to_string().contains("multiple API key sources"));
    }

    #[cfg(feature = "keyring")]
    #[test]
    fn resolve_api_key_from_keyring() {
        // Install mock backend (idempotent across tests).
        ::keyring::set_default_credential_builder(::keyring::mock::default_credential_builder());
        crate::credentials::keyring::store("bzr", "resolve-test-srv1", "keyring-secret").unwrap();

        let server = ServerConfig {
            url: "https://example.com".into(),
            api_key: None,
            api_key_env: None,
            api_key_keyring: Some(KeyringRef {
                service: None,
                account: Some("resolve-test-srv1".into()),
            }),
            email: None,
            auth_method: None,
            api_mode: None,
            server_version: None,
            tls_insecure: false,
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_issuer: None,
        };

        assert_eq!(
            server.resolve_api_key("resolve-test-srv1").unwrap(),
            "keyring-secret"
        );

        // cleanup
        crate::credentials::keyring::delete("bzr", "resolve-test-srv1").unwrap();
    }

    #[cfg(feature = "keyring")]
    #[test]
    fn resolve_api_key_from_keyring_with_explicit_service_and_account() {
        ::keyring::set_default_credential_builder(::keyring::mock::default_credential_builder());
        crate::credentials::keyring::store(
            "resolve-test-myservice",
            "resolve-test-myacct",
            "explicit-secret",
        )
        .unwrap();

        let server = ServerConfig {
            url: "https://example.com".into(),
            api_key: None,
            api_key_env: None,
            api_key_keyring: Some(KeyringRef {
                service: Some("resolve-test-myservice".into()),
                account: Some("resolve-test-myacct".into()),
            }),
            email: None,
            auth_method: None,
            api_mode: None,
            server_version: None,
            tls_insecure: false,
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_issuer: None,
        };

        assert_eq!(
            server.resolve_api_key("any-name").unwrap(),
            "explicit-secret"
        );

        crate::credentials::keyring::delete("resolve-test-myservice", "resolve-test-myacct")
            .unwrap();
    }

    #[cfg(feature = "keyring")]
    #[test]
    fn resolve_api_key_from_keyring_defaults_account_to_server_name() {
        ::keyring::set_default_credential_builder(::keyring::mock::default_credential_builder());
        // Store under the server name (no explicit account).
        crate::credentials::keyring::store("bzr", "resolve-test-srv2", "default-account-secret")
            .unwrap();

        let server = ServerConfig {
            url: "https://example.com".into(),
            api_key: None,
            api_key_env: None,
            // Neither service nor account set — should default to ("bzr", server_name).
            api_key_keyring: Some(KeyringRef {
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
        };

        assert_eq!(
            server.resolve_api_key("resolve-test-srv2").unwrap(),
            "default-account-secret"
        );

        crate::credentials::keyring::delete("bzr", "resolve-test-srv2").unwrap();
    }

    #[test]
    fn validate_tls_insecure_with_ca_cert_conflicts() {
        let server = ServerConfig {
            url: "https://example.com".into(),
            api_key: Some("key".into()),
            api_key_env: None,
            api_key_keyring: None,
            email: None,
            auth_method: None,
            api_mode: None,
            server_version: None,
            tls_insecure: true,
            tls_ca_cert: Some(PathBuf::from("/tmp/ca.pem")),
            tls_pin_sha256: None,
            tls_pin_issuer: None,
        };
        let err = server.validate_tls("srv").unwrap_err();
        assert!(
            err.to_string().contains("mutually exclusive"),
            "error should mention 'mutually exclusive': {err}"
        );
    }

    #[test]
    fn validate_tls_insecure_with_pin_conflicts() {
        let server = ServerConfig {
            url: "https://example.com".into(),
            api_key: Some("key".into()),
            api_key_env: None,
            api_key_keyring: None,
            email: None,
            auth_method: None,
            api_mode: None,
            server_version: None,
            tls_insecure: true,
            tls_ca_cert: None,
            tls_pin_sha256: Some("sha256//abc".into()),
            tls_pin_issuer: None,
        };
        let err = server.validate_tls("srv").unwrap_err();
        assert!(
            err.to_string().contains("mutually exclusive"),
            "error should mention 'mutually exclusive': {err}"
        );
    }

    #[test]
    fn validate_tls_ca_cert_with_pin_conflicts() {
        let server = ServerConfig {
            url: "https://example.com".into(),
            api_key: Some("key".into()),
            api_key_env: None,
            api_key_keyring: None,
            email: None,
            auth_method: None,
            api_mode: None,
            server_version: None,
            tls_insecure: false,
            tls_ca_cert: Some(PathBuf::from("/tmp/ca.pem")),
            tls_pin_sha256: Some("sha256//abc".into()),
            tls_pin_issuer: None,
        };
        let err = server.validate_tls("srv").unwrap_err();
        assert!(
            err.to_string().contains("mutually exclusive"),
            "error should mention 'mutually exclusive': {err}"
        );
    }

    #[test]
    fn validate_tls_no_conflicts_passes() {
        let server = ServerConfig {
            url: "https://example.com".into(),
            api_key: Some("key".into()),
            api_key_env: None,
            api_key_keyring: None,
            email: None,
            auth_method: None,
            api_mode: None,
            server_version: None,
            tls_insecure: false,
            tls_ca_cert: None,
            tls_pin_sha256: None,
            tls_pin_issuer: None,
        };
        assert!(server.validate_tls("srv").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn config_save_hardens_permissions_for_new_paths() {
        let _lock = crate::ENV_LOCK.blocking_lock();
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: Tests are serialized via ENV_LOCK; no other threads read this var concurrently.
        unsafe { env::set_var("XDG_CONFIG_HOME", tmp.path()) };

        let config = make_config_with_server();
        config.save().unwrap();

        let config_dir = tmp.path().join("bzr");
        let config_path = config_dir.join("config.toml");
        let dir_mode = fs::metadata(config_dir).unwrap().permissions().mode() & 0o777;
        let file_mode = fs::metadata(config_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700);
        assert_eq!(file_mode, 0o600);
    }
}
