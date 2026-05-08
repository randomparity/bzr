#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn store_retrieve_delete_roundtrip() {
    install_test_store();
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
    install_test_store();
    let err = retrieve("bzr-test-missing", "missing-account").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("no API key found"), "got: {msg}");
}

#[test]
fn delete_missing_entry_is_ok() {
    install_test_store();
    // Idempotent: the test store returns NoEntry for missing entries, which
    // the wrapper maps to Ok.
    delete("bzr-test-delete", "never-existed").unwrap();
}
