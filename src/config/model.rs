use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{BzrError, Result};
use crate::types::common::{ApiMode, AuthMethod};
use crate::types::query::SavedQuery;
use crate::types::template::BugTemplate;

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// Base64-encoded raw DER bytes of the issuer SEQUENCE for
    /// tamper-proof issuer comparison.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin_issuer_der: Option<String>,
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
    Keyring {
        service: &'a str,
        account: KeyringAccount<'a>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyringAccount<'a> {
    /// Use this exact keyring account string.
    Explicit(&'a str),
    /// Resolve the keyring account from the server name at lookup time.
    ServerDefault,
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
    /// Build an ephemeral server backed by an environment-variable credential,
    /// for the inline `--server-url` flow. The result is never written to disk:
    /// auth method and API mode are left unset (detected per-invocation) and TLS
    /// uses the default OS trust store. Construction cannot fail; an unset or
    /// empty env var surfaces later from [`Self::resolve_api_key`].
    #[must_use]
    pub fn from_url_with_env_key(url: String, api_key_env: String, email: Option<String>) -> Self {
        ServerConfig {
            url,
            api_key_env: Some(api_key_env),
            email,
            ..Self::default()
        }
    }

    pub fn tls_config(&self, server_name: &str) -> crate::tls::TlsConfig {
        crate::tls::TlsConfig {
            insecure: self.tls_insecure,
            ca_cert_path: self.tls_ca_cert.clone(),
            pin_sha256: self.tls_pin_sha256.clone(),
            pin_issuer_der: self.tls_pin_issuer_der.clone(),
            server_name: Some(server_name.to_string()),
        }
    }

    pub fn validate(&self, server_name: &str) -> Result<()> {
        self.credential_source()
            .map(|_| ())
            .map_err(|err| BzrError::config(format!("server '{server_name}': {err}")))?;
        self.validate_tls(server_name)
    }

    pub fn credential_source(&self) -> Result<Option<CredentialSource<'_>>> {
        let count = usize::from(self.api_key.is_some())
            + usize::from(self.api_key_env.is_some())
            + usize::from(self.api_key_keyring.is_some());
        match count {
            0 => Ok(None),
            1 => {
                if let Some(api_key) = self.api_key.as_deref() {
                    Ok(Some(CredentialSource::Inline(api_key)))
                } else if let Some(var_name) = self.api_key_env.as_deref() {
                    Ok(Some(CredentialSource::EnvVar(var_name)))
                } else {
                    let r = self.api_key_keyring.as_ref().ok_or_else(|| {
                        BzrError::config("internal: keyring credential unexpectedly missing")
                    })?;
                    let account = r
                        .account
                        .as_deref()
                        .map_or(KeyringAccount::ServerDefault, KeyringAccount::Explicit);
                    Ok(Some(CredentialSource::Keyring {
                        service: r.service_or_default(),
                        account,
                    }))
                }
            }
            _ => Err(BzrError::config(
                "server config cannot define multiple API key sources \
                 (api_key, api_key_env, api_key_keyring)",
            )),
        }
    }

    pub fn credential_source_kind(&self) -> Result<Option<CredentialSourceKind>> {
        Ok(self.credential_source()?.map(|source| source.kind()))
    }

    pub fn resolve_optional_api_key(&self, server_name: &str) -> Result<Option<String>> {
        match self.credential_source()? {
            Some(CredentialSource::Inline(api_key)) => Ok(Some(api_key.to_string())),
            Some(CredentialSource::EnvVar(var_name)) => {
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
                Ok(Some(value))
            }
            Some(CredentialSource::Keyring { service, account }) => {
                let account = match account {
                    KeyringAccount::Explicit(account) => account,
                    KeyringAccount::ServerDefault => server_name,
                };
                crate::credentials::keyring::retrieve(service, account).map(Some)
            }
            None => Ok(None),
        }
    }

    pub fn resolve_api_key(&self, server_name: &str) -> Result<String> {
        self.resolve_optional_api_key(server_name)?.ok_or_else(|| {
            BzrError::config(format!(
                "server '{server_name}' has no API key source configured"
            ))
        })
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
        if let Some(path) = &self.tls_ca_cert {
            if !path.exists() {
                return Err(BzrError::config(format!(
                    "server '{server_name}': tls_ca_cert file not found: {}",
                    path.display()
                )));
            }
        }
        if let Some(pin) = &self.tls_pin_sha256 {
            crate::tls::fingerprint::parse_pin(pin)
                .map_err(|e| ctx(&format!("invalid tls_pin_sha256: {e}")))?;
        }
        Ok(())
    }
}

impl Config {
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

    pub(super) fn validate(&self) -> Result<()> {
        for (name, server) in &self.servers {
            server.validate(name)?;
        }
        Ok(())
    }
}
