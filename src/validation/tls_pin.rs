use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;

use crate::error::{BzrError, Result};

pub(crate) const SHA256_PIN_PREFIX: &str = "sha256//";

/// Parse a `sha256//<base64>` pin string into a 32-byte SHA-256 hash.
pub(crate) fn parse_sha256_pin(pin: &str) -> Result<[u8; 32]> {
    let b64 = pin.strip_prefix(SHA256_PIN_PREFIX).ok_or_else(|| {
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
#[path = "tls_pin_tests.rs"]
mod tests;
