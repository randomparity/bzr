use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use sha2::{Digest, Sha256};

use crate::error::{BzrError, Result};

const PIN_PREFIX: &str = "sha256//";

/// Compute a SHA-256 certificate fingerprint in `sha256//<base64>` format.
///
/// The input is a DER-encoded certificate (or any raw bytes). The output
/// matches the HPKP / TLS certificate pinning pin format.
pub(crate) fn compute_fingerprint(der: &[u8]) -> String {
    let hash = Sha256::digest(der);
    format!("{PIN_PREFIX}{}", BASE64_STANDARD.encode(hash))
}

/// Parse a `sha256//<base64>` pin string into a 32-byte SHA-256 hash.
///
/// Returns `InputValidation` errors for:
/// - missing `sha256//` prefix
/// - invalid base64 encoding
/// - decoded length that is not exactly 32 bytes
pub(crate) fn parse_pin(pin: &str) -> Result<[u8; 32]> {
    let b64 = pin.strip_prefix(PIN_PREFIX).ok_or_else(|| {
        BzrError::InputValidation(format!("pin must start with `sha256//`: {pin}"))
    })?;

    let decoded = BASE64_STANDARD
        .decode(b64)
        .map_err(|e| BzrError::InputValidation(format!("pin has invalid base64 encoding: {e}")))?;

    decoded.try_into().map_err(|v: Vec<u8>| {
        BzrError::InputValidation(format!("pin decoded to {} bytes, expected 32", v.len()))
    })
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn compute_fingerprint_deterministic() {
        let input = b"example certificate bytes";
        let fp1 = compute_fingerprint(input);
        let fp2 = compute_fingerprint(input);
        assert_eq!(fp1, fp2);
        assert!(
            fp1.starts_with("sha256//"),
            "fingerprint must start with sha256//"
        );
    }

    #[test]
    fn compute_fingerprint_format() {
        let fp = compute_fingerprint(b"test data");
        assert!(
            fp.starts_with("sha256//"),
            "fingerprint must start with sha256//"
        );
        let b64_part = fp.strip_prefix("sha256//").unwrap();
        let decoded = BASE64_STANDARD.decode(b64_part);
        assert!(
            decoded.is_ok(),
            "base64 portion must decode successfully: {decoded:?}"
        );
    }

    #[test]
    fn round_trip_through_parse_pin() {
        let input = b"some DER-encoded cert";
        let expected_hash: [u8; 32] = Sha256::digest(input).into();
        let fp = compute_fingerprint(input);
        let parsed = parse_pin(&fp).unwrap();
        assert_eq!(parsed, expected_hash);
    }

    #[test]
    fn parse_pin_rejects_bad_prefix() {
        let result = parse_pin("md5//abcdef");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("sha256//"),
            "error should mention sha256//: {msg}"
        );
    }

    #[test]
    fn parse_pin_rejects_bad_base64() {
        let result = parse_pin("sha256//!!!not-valid-base64!!!");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("base64"), "error should mention base64: {msg}");
    }

    #[test]
    fn parse_pin_rejects_wrong_length() {
        // Valid base64, but only 4 bytes when decoded — not 32
        let short = BASE64_STANDARD.encode(b"abc");
        let pin = format!("sha256//{short}");
        let result = parse_pin(&pin);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("32"),
            "error should mention expected length 32: {msg}"
        );
    }
}
