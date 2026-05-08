//! OS keychain wrapper around `keyring-core` and native store crates.
//!
//! Maps `keyring_core::Error` variants to user-facing `BzrError::Keyring`
//! messages so callers get actionable guidance on failures.
//!
//! Entries are cached by `(service, account)` to give every
//! operation on the same key a stable handle. In production, this is
//! a small optimization — `Entry` is a thin wrapper over a platform
//! credential and each method call reaches the backend anyway.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use keyring_core::{CredentialStore, Entry, Error as KrError};

use crate::error::{BzrError, Result};

type EntryCache = Mutex<HashMap<(String, String), Arc<Entry>>>;

fn cache() -> &'static EntryCache {
    static CACHE: OnceLock<EntryCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn entry_for(service: &str, account: &str) -> Result<Arc<Entry>> {
    ensure_default_store(service, account)?;
    let mut guard = cache()
        .lock()
        .map_err(|e| BzrError::Keyring(format!("keychain entry cache poisoned: {e}")))?;
    let key = (service.to_string(), account.to_string());
    if let Some(entry) = guard.get(&key) {
        return Ok(Arc::clone(entry));
    }
    let entry = Entry::new(service, account).map_err(|e| {
        BzrError::Keyring(format!(
            "failed to open keychain entry for service='{service}' account='{account}': {e}"
        ))
    })?;
    let arc = Arc::new(entry);
    guard.insert(key, Arc::clone(&arc));
    Ok(arc)
}

fn ensure_default_store(service: &str, account: &str) -> Result<()> {
    if keyring_core::get_default_store().is_some() {
        return Ok(());
    }

    let store = native_store().map_err(|e| {
        BzrError::Keyring(format!(
            "failed to initialize OS keychain store for service='{service}' \
             account='{account}': {e}"
        ))
    })?;
    keyring_core::set_default_store(store);
    Ok(())
}

#[cfg(target_os = "macos")]
fn native_store() -> keyring_core::Result<Arc<CredentialStore>> {
    let store: Arc<CredentialStore> = apple_native_keyring_store::keychain::Store::new()?;
    Ok(store)
}

#[cfg(target_os = "linux")]
fn native_store() -> keyring_core::Result<Arc<CredentialStore>> {
    let store: Arc<CredentialStore> = dbus_secret_service_keyring_store::Store::new()?;
    Ok(store)
}

#[cfg(target_os = "windows")]
fn native_store() -> keyring_core::Result<Arc<CredentialStore>> {
    let store: Arc<CredentialStore> = windows_native_keyring_store::Store::new()?;
    Ok(store)
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn native_store() -> keyring_core::Result<Arc<CredentialStore>> {
    Err(KrError::NotSupportedByStore(
        "OS keychain storage is supported on Linux, macOS, and Windows".to_string(),
    ))
}

/// Store a secret in the OS keychain at `(service, account)`.
pub fn store(service: &str, account: &str, secret: &str) -> Result<()> {
    let entry = entry_for(service, account)?;
    entry
        .set_password(secret)
        .map_err(|e| map_error(service, account, &e))
}

/// Retrieve a secret from the OS keychain at `(service, account)`.
pub fn retrieve(service: &str, account: &str) -> Result<String> {
    let entry = entry_for(service, account)?;
    entry
        .get_password()
        .map_err(|e| map_error(service, account, &e))
}

/// Delete a secret from the OS keychain. Missing entries are not an error.
pub fn delete(service: &str, account: &str) -> Result<()> {
    let entry = entry_for(service, account)?;
    match entry.delete_credential() {
        Ok(()) | Err(KrError::NoEntry) => Ok(()),
        Err(e) => Err(map_error(service, account, &e)),
    }
}

fn map_error(service: &str, account: &str, err: &KrError) -> BzrError {
    let message = match err {
        KrError::NoEntry => format!(
            "no API key found in OS keychain for service='{service}' account='{account}'. \
             Run `bzr config set-keyring <server>` to store one."
        ),
        KrError::PlatformFailure(inner) => format!(
            "OS keychain unavailable: {inner}. \
             For headless/CI environments, use api_key_env instead — see docs/bzr-cli.md."
        ),
        KrError::NoStorageAccess(inner) => format!(
            "OS keychain locked or inaccessible: {inner}. \
             For headless/CI environments, use api_key_env instead — see docs/bzr-cli.md."
        ),
        KrError::TooLong(attr, limit) => format!(
            "keychain attribute '{attr}' exceeds platform limit of {limit} characters \
             (service='{service}', account='{account}'). Shorten the server name or \
             override via --service/--account."
        ),
        KrError::Ambiguous(_) => format!(
            "multiple matching keychain entries for service='{service}' account='{account}'; \
             please remove duplicates."
        ),
        KrError::BadEncoding(_) | KrError::Invalid(..) => format!(
            "stored keychain entry for service='{service}' account='{account}' is corrupted: {err}"
        ),
        other => format!("keychain error: {other}"),
    };
    BzrError::Keyring(message)
}

#[cfg(test)]
pub(crate) fn install_test_store() {
    let store: Arc<CredentialStore> = Arc::new(test_store::Store::default());
    keyring_core::set_default_store(store);
}

#[cfg(test)]
mod test_store {
    use std::any::Any;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use keyring_core::api::{CredentialApi, CredentialPersistence, CredentialStoreApi};
    use keyring_core::{Credential, Entry, Error, Result};

    type Secrets = Arc<Mutex<HashMap<(String, String), Vec<u8>>>>;

    #[derive(Debug, Default)]
    pub(crate) struct Store {
        secrets: Secrets,
    }

    impl CredentialStoreApi for Store {
        fn vendor(&self) -> String {
            "bzr test keyring store".to_string()
        }

        fn id(&self) -> String {
            "bzr-test-keyring-store".to_string()
        }

        fn build(
            &self,
            service: &str,
            user: &str,
            _modifiers: Option<&HashMap<&str, &str>>,
        ) -> Result<Entry> {
            Ok(Entry::new_with_credential(Arc::new(TestCredential {
                service: service.to_string(),
                user: user.to_string(),
                secrets: Arc::clone(&self.secrets),
            })))
        }

        fn as_any(&self) -> &dyn Any {
            self
        }

        fn persistence(&self) -> CredentialPersistence {
            CredentialPersistence::ProcessOnly
        }
    }

    #[derive(Clone, Debug)]
    struct TestCredential {
        service: String,
        user: String,
        secrets: Secrets,
    }

    impl TestCredential {
        fn key(&self) -> (String, String) {
            (self.service.clone(), self.user.clone())
        }

        fn lock_secrets(
            &self,
        ) -> Result<std::sync::MutexGuard<'_, HashMap<(String, String), Vec<u8>>>> {
            self.secrets.lock().map_err(|e| {
                Error::PlatformFailure(Box::new(std::io::Error::other(format!(
                    "test keyring store poisoned: {e}"
                ))))
            })
        }
    }

    impl CredentialApi for TestCredential {
        fn set_secret(&self, secret: &[u8]) -> Result<()> {
            let mut secrets = self.lock_secrets()?;
            secrets.insert(self.key(), secret.to_vec());
            Ok(())
        }

        fn get_secret(&self) -> Result<Vec<u8>> {
            let secrets = self.lock_secrets()?;
            secrets.get(&self.key()).cloned().ok_or(Error::NoEntry)
        }

        fn delete_credential(&self) -> Result<()> {
            let mut secrets = self.lock_secrets()?;
            if secrets.remove(&self.key()).is_some() {
                Ok(())
            } else {
                Err(Error::NoEntry)
            }
        }

        fn get_credential(&self) -> Result<Option<Arc<Credential>>> {
            let secrets = self.lock_secrets()?;
            if secrets.contains_key(&self.key()) {
                Ok(Some(Arc::new(self.clone())))
            } else {
                Err(Error::NoEntry)
            }
        }

        fn get_specifiers(&self) -> Option<(String, String)> {
            Some(self.key())
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }
}

#[cfg(test)]
#[path = "keyring_tests.rs"]
mod tests;
