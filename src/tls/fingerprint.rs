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
#[path = "fingerprint_tests.rs"]
mod tests;
