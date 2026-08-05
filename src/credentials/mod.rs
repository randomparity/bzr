//! Credential storage backends.
//!
//! Currently provides a single backend: the OS keychain, via the
//! [`keyring`] crate. Gated behind the `keyring` Cargo feature;
//! when disabled, a stub returns clear "unsupported" errors so the
//! binary still parses keyring-backed config entries.

use crate::config::{CredentialSource, KeyringAccount, ServerConfig};
use crate::error::{BzrError, Result};

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
        Some(source) => resolve_source(source, server_name).map(Some),
        None => Ok(None),
    }?;
    if let Some(api_key) = api_key.as_deref() {
        crate::bugzilla_auth::register_active_api_key(api_key);
    }
    Ok(api_key)
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
    }
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
