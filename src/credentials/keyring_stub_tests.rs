#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn store_returns_unsupported() {
    let err = store("s", "a", "v").unwrap_err();
    assert!(err.to_string().contains("compiled without keyring support"));
}

#[test]
fn retrieve_returns_unsupported() {
    let err = retrieve("s", "a").unwrap_err();
    assert!(err.to_string().contains("compiled without keyring support"));
}

#[test]
fn delete_returns_unsupported() {
    let err = delete("s", "a").unwrap_err();
    assert!(err.to_string().contains("compiled without keyring support"));
}
