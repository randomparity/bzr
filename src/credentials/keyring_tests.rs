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

#[test]
fn install_test_store_is_idempotent_across_repeated_calls() {
    // CONC-4: the default-store init is check-then-act; under the
    // current_thread runtime it is benign and idempotent. Calling it
    // repeatedly must remain safe and leave a usable default store.
    install_test_store();
    install_test_store();
    assert!(
        keyring_core::get_default_store().is_some(),
        "a default store must be installed after repeated init"
    );
}
