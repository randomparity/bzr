use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use sha2::{Digest, Sha256};

/// Compute a SHA-256 certificate fingerprint in `sha256//<base64>` format.
///
/// The input is a DER-encoded certificate (or any raw bytes). The output
/// matches the HPKP / TLS certificate pinning pin format.
pub(crate) fn compute_fingerprint(der: &[u8]) -> String {
    let hash = Sha256::digest(der);
    format!(
        "{}{}",
        crate::validation::SHA256_PIN_PREFIX,
        BASE64_STANDARD.encode(hash)
    )
}

#[cfg(test)]
#[path = "fingerprint_tests.rs"]
mod tests;
