//! OS keychain wrapper around the `keyring` crate.
//!
//! Maps `keyring::Error` variants to user-facing `BzrError::Keyring`
//! messages so callers get actionable guidance on failures.

use ::keyring::{Entry, Error as KrError};

use crate::error::{BzrError, Result};

/// Store a secret in the OS keychain at `(service, account)`.
pub fn store(service: &str, account: &str, secret: &str) -> Result<()> {
    let entry = new_entry(service, account)?;
    entry
        .set_password(secret)
        .map_err(|e| map_error(service, account, &e))
}

/// Retrieve a secret from the OS keychain at `(service, account)`.
pub fn retrieve(service: &str, account: &str) -> Result<String> {
    let entry = new_entry(service, account)?;
    entry
        .get_password()
        .map_err(|e| map_error(service, account, &e))
}

/// Delete a secret from the OS keychain. Missing entries are not an error.
pub fn delete(service: &str, account: &str) -> Result<()> {
    let entry = new_entry(service, account)?;
    match entry.delete_credential() {
        Ok(()) | Err(KrError::NoEntry) => Ok(()),
        Err(e) => Err(map_error(service, account, &e)),
    }
}

fn new_entry(service: &str, account: &str) -> Result<Entry> {
    Entry::new(service, account).map_err(|e| {
        BzrError::Keyring(format!(
            "failed to open keychain entry for service='{service}' account='{account}': {e}"
        ))
    })
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

    fn install_mock() {
        // Idempotent: subsequent calls are no-ops.
        ::keyring::set_default_credential_builder(::keyring::mock::default_credential_builder());
    }

    #[test]
    fn store_retrieve_delete_roundtrip() {
        // The mock backend uses CredentialPersistence::EntryOnly — secrets are
        // stored in the Entry object, not in a shared in-process store.  We
        // must therefore operate through the same Entry instance for the whole
        // roundtrip instead of calling the public store/retrieve/delete
        // helpers, which each create a fresh Entry.
        install_mock();
        let entry = new_entry("bzr-test", "acct1").unwrap();
        entry.set_password("secret-value").unwrap();
        let got = entry.get_password().unwrap();
        assert_eq!(got, "secret-value");
        entry.delete_credential().unwrap();
        let err = entry.get_password().unwrap_err();
        assert!(matches!(err, ::keyring::Error::NoEntry));
    }

    #[test]
    fn retrieve_missing_entry_maps_to_no_entry_message() {
        install_mock();
        let err = retrieve("bzr-test", "missing-account").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("no API key found"), "got: {msg}");
    }

    #[test]
    fn delete_missing_entry_is_ok() {
        install_mock();
        // Idempotent
        delete("bzr-test", "never-existed").unwrap();
    }
}
