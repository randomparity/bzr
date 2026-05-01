//! OS keychain wrapper around the `keyring` crate.
//!
//! Maps `keyring::Error` variants to user-facing `BzrError::Keyring`
//! messages so callers get actionable guidance on failures.
//!
//! Entries are cached by `(service, account)` to give every
//! operation on the same key a stable handle. In production, this is
//! a small optimization — `Entry` is a thin wrapper over a platform
//! credential and each method call reaches the backend anyway. In
//! tests that install `keyring::mock::default_credential_builder()`,
//! the cache is essential: the v3 mock uses `EntryOnly` persistence,
//! so store and retrieve must see the same `Entry` instance to share
//! state.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use ::keyring::{Entry, Error as KrError};

use crate::error::{BzrError, Result};

type EntryCache = Mutex<HashMap<(String, String), Arc<Entry>>>;

fn cache() -> &'static EntryCache {
    static CACHE: OnceLock<EntryCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn entry_for(service: &str, account: &str) -> Result<Arc<Entry>> {
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
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    // Tests share a process-wide cache of Arc<Entry> keyed by
    // (service, account). Each test must use a unique pair to avoid
    // hitting another test's state. install_mock() is idempotent across
    // tests, so calling it in every test is safe.
    fn install_mock() {
        use std::sync::OnceLock;
        static INSTALLED: OnceLock<()> = OnceLock::new();
        INSTALLED.get_or_init(|| {
            ::keyring::set_default_credential_builder(
                ::keyring::mock::default_credential_builder(),
            );
        });
    }

    #[test]
    fn store_retrieve_delete_roundtrip() {
        install_mock();
        store("bzr-test-roundtrip", "acct1", "secret-value").unwrap();
        let got = retrieve("bzr-test-roundtrip", "acct1").unwrap();
        assert_eq!(got, "secret-value");
        delete("bzr-test-roundtrip", "acct1").unwrap();
        // Retrieve must now fail; otherwise delete was a no-op.
        let err = retrieve("bzr-test-roundtrip", "acct1").unwrap_err();
        assert!(
            err.to_string().contains("no API key found"),
            "expected NoEntry after delete, got: {err}"
        );
    }

    #[test]
    fn retrieve_missing_entry_maps_to_no_entry_message() {
        install_mock();
        let err = retrieve("bzr-test-missing", "missing-account").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("no API key found"), "got: {msg}");
    }

    #[test]
    fn delete_missing_entry_is_ok() {
        install_mock();
        // Idempotent: mock returns NoEntry for missing entries, which
        // the wrapper maps to Ok.
        delete("bzr-test-delete", "never-existed").unwrap();
    }
}
