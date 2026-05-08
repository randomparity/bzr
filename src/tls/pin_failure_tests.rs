#![expect(clippy::panic)]

use super::*;

#[test]
fn classifies_pin_mismatch_chain_into_typed_error() {
    let chain = "error sending request: PIN_MISMATCH for test: \
                 expected sha256//old==, got sha256//new==, issuer CN=Test CA";
    let failure = classify_chain(chain);

    let Some(TlsPinFailure::PinMismatch {
        expected,
        actual,
        new_issuer,
    }) = failure
    else {
        panic!("expected pin mismatch failure");
    };
    assert_eq!(expected, "sha256//old==");
    assert_eq!(actual, "sha256//new==");
    assert_eq!(new_issuer, "CN=Test CA");
}

#[test]
fn classifies_legacy_issuer_change_chain_into_typed_error() {
    let chain = "error sending request: ISSUER_CHANGED for test: \
                 expected \"CN=Good\", got \"CN=Bad\"";
    let failure = classify_chain(chain);

    let Some(TlsPinFailure::IssuerChanged {
        expected_issuer,
        actual_issuer,
    }) = failure
    else {
        panic!("expected issuer changed failure");
    };
    assert_eq!(expected_issuer, "CN=Good");
    assert_eq!(actual_issuer, "CN=Bad");
}

#[test]
fn classifies_der_issuer_change_chain_into_typed_error() {
    let chain = "error sending request: ISSUER_CHANGED for test: issuer DER mismatch \
                 (expected 64 bytes, got 65 bytes)";
    let failure = classify_chain(chain);

    let Some(TlsPinFailure::IssuerChanged {
        expected_issuer,
        actual_issuer,
    }) = failure
    else {
        panic!("expected issuer changed failure");
    };
    assert_eq!(expected_issuer, "pinned issuer DER");
    assert_eq!(actual_issuer, "presented issuer DER");
}

#[test]
fn unrelated_chain_is_not_a_pin_failure() {
    assert!(classify_chain("connection refused").is_none());
}
