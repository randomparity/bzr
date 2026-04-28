pub(crate) mod fingerprint;
#[cfg_attr(not(test), expect(dead_code, reason = "consumed by later TLS tasks"))]
pub(crate) mod verifier;
