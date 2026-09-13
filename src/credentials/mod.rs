//! Credential storage backends.
//!
//! Currently provides a single backend: the OS keychain, via the
//! [`keyring`] crate. Gated behind the `keyring` Cargo feature;
//! when disabled, a stub returns clear "unsupported" errors so the
//! binary still parses keyring-backed config entries.

use crate::config::{CredentialSource, KeyringAccount, ServerConfig};
use crate::error::{BzrError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedCredential {
    ApiKey(String),
    Token(String),
}

impl ResolvedCredential {
    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::ApiKey(value) | Self::Token(value) => value,
        }
    }
}

#[cfg(feature = "keyring")]
pub mod keyring;

#[cfg(not(feature = "keyring"))]
#[path = "keyring_stub.rs"]
pub mod keyring;

pub(crate) fn resolve_optional_api_key(
    server: &ServerConfig,
    server_name: &str,
) -> Result<Option<String>> {
    let source = server.credential_source()?;
    let api_key = match source.as_ref() {
        Some(CredentialSource::Token(_)) => {
            return Err(BzrError::config(format!(
                "server '{server_name}' uses a login token, not an API key"
            )));
        }
        Some(source) => resolve_source(source, server_name).map(Some),
        None => Ok(None),
    }?;
    if let Some(api_key) = api_key.as_deref() {
        crate::bugzilla_auth::register_active_api_key(api_key);
    }
    Ok(api_key)
}

pub(crate) fn resolve_optional_credential(
    server: &ServerConfig,
    server_name: &str,
) -> Result<Option<ResolvedCredential>> {
    let source = server.credential_source()?;
    let credential = match source.as_ref() {
        Some(CredentialSource::Token(token)) => {
            Ok(Some(ResolvedCredential::Token((*token).into())))
        }
        Some(source) => {
            resolve_source(source, server_name).map(|value| Some(ResolvedCredential::ApiKey(value)))
        }
        None => Ok(None),
    }?;
    if let Some(credential) = &credential {
        crate::bugzilla_auth::register_active_credential(credential.as_str());
    }
    Ok(credential)
}

pub(crate) fn resolve_api_key(server: &ServerConfig, server_name: &str) -> Result<String> {
    resolve_optional_api_key(server, server_name)?.ok_or_else(|| {
        BzrError::config(format!(
            "server '{server_name}' has no API key source configured"
        ))
    })
}

fn resolve_source(source: &CredentialSource<'_>, server_name: &str) -> Result<String> {
    match source {
        CredentialSource::Inline(api_key) => Ok((*api_key).to_string()),
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
            let account = match account {
                KeyringAccount::Explicit(account) => *account,
                KeyringAccount::ServerDefault => server_name,
            };
            keyring::retrieve(service, account)
        }
        CredentialSource::Token(_) => unreachable!("tokens are resolved directly"),
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
