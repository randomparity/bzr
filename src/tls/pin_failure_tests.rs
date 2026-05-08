#![expect(clippy::panic)]

use super::*;

#[test]
fn classifies_pin_mismatch_chain_into_typed_error() {
    let chain = "error sending request: PIN_MISMATCH for test: \
                 expected sha256//old==, got sha256//new==, issuer CN=Test CA";
    let failure = classify_chain(chain, "test");

    let Some(TlsPinFailure::PinMismatch { error, new_issuer }) = failure else {
        panic!("expected pin mismatch failure");
    };
    assert!(matches!(
        error,
        BzrError::PinMismatch {
            ref server,
            ref expected,
            ref actual,
        } if server == "test" && expected == "sha256//old==" && actual == "sha256//new=="
    ));
    assert_eq!(new_issuer, "CN=Test CA");
}

#[test]
fn classifies_legacy_issuer_change_chain_into_typed_error() {
    let chain = "error sending request: ISSUER_CHANGED for test: \
                 expected \"CN=Good\", got \"CN=Bad\"";
    let failure = classify_chain(chain, "test");

    let Some(TlsPinFailure::IssuerChanged(error)) = failure else {
        panic!("expected issuer changed failure");
    };
    assert!(matches!(
        error,
        BzrError::IssuerChanged {
            ref server,
            ref expected_issuer,
            ref actual_issuer,
        } if server == "test" && expected_issuer == "CN=Good" && actual_issuer == "CN=Bad"
    ));
}

#[test]
fn classifies_der_issuer_change_chain_into_typed_error() {
    let chain = "error sending request: ISSUER_CHANGED for test: issuer DER mismatch \
                 (expected 64 bytes, got 65 bytes)";
    let failure = classify_chain(chain, "test");

    let Some(TlsPinFailure::IssuerChanged(error)) = failure else {
        panic!("expected issuer changed failure");
    };
    assert!(matches!(
        error,
        BzrError::IssuerChanged {
            ref expected_issuer,
            ref actual_issuer,
            ..
        } if expected_issuer == "pinned issuer DER" && actual_issuer == "presented issuer DER"
    ));
}

#[test]
fn unrelated_chain_is_not_a_pin_failure() {
    assert!(classify_chain("connection refused", "test").is_none());
}
