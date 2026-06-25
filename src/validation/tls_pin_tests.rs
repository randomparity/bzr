#![expect(clippy::unwrap_used)]

use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use sha2::{Digest as _, Sha256};

use super::*;

#[test]
fn round_trip_sha256_pin() {
    let input = b"some DER-encoded cert";
    let expected_hash: [u8; 32] = Sha256::digest(input).into();
    let pin = format!(
        "{SHA256_PIN_PREFIX}{}",
        BASE64_STANDARD.encode(expected_hash)
    );
    let parsed = parse_sha256_pin(&pin).unwrap();
    assert_eq!(parsed, expected_hash);
}

#[test]
fn parse_sha256_pin_rejects_bad_prefix() {
    let result = parse_sha256_pin("md5//abcdef");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("sha256//"),
        "error should mention sha256//: {msg}"
    );
}

#[test]
fn parse_sha256_pin_rejects_bad_base64() {
    let result = parse_sha256_pin("sha256//!!!not-valid-base64!!!");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("base64"), "error should mention base64: {msg}");
}

#[test]
fn parse_sha256_pin_rejects_wrong_length() {
    let short = BASE64_STANDARD.encode(b"abc");
    let pin = format!("{SHA256_PIN_PREFIX}{short}");
    let result = parse_sha256_pin(&pin);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("32"),
        "error should mention expected length 32: {msg}"
    );
}
