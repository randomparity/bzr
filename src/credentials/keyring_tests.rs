#![expect(clippy::unwrap_used)]

use super::*;

// Tests share a process-wide cache of Arc<Entry> keyed by
// (service, account). Each test must use a unique pair to avoid
// hitting another test's state. install_mock() is idempotent across
// tests, so calling it in every test is safe.
fn install_mock() {
    use std::sync::OnceLock;
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        ::keyring::set_default_credential_builder(::keyring::mock::default_credential_builder());
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
