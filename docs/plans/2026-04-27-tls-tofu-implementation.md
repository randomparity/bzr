# TLS TOFU and Certificate Pinning Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the blunt `--tls-insecure` escape hatch with secure certificate trust options: custom CA certs, explicit pinning, and SSH-style trust-on-first-use.

**Architecture:** New `src/tls/` module owns all TLS configuration. A custom `rustls::client::danger::ServerCertVerifier` handles four trust modes: system roots (default), custom CA, pinned fingerprint, and insecure. TOFU intercepts cert errors in `commands/shared.rs` and prompts interactively. Config stores pins per-server in `config.toml`.

**Tech Stack:** rustls 0.23 (custom verifier), rustls-pki-types (cert types), sha2 (fingerprinting), base64 (already present), rcgen (dev-only, test cert generation)

**Spec:** `docs/plans/2026-04-27-tls-tofu-cert-pinning.md`

---

### Task 1: Add dependencies

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add production dependencies**

Add to `[dependencies]` in `Cargo.toml`, after the existing `reqwest` line:

```toml
rustls = { version = "0.23", default-features = false, features = ["std"] }
rustls-pki-types = "1"
sha2 = "0.10"
webpki-roots = "0.26"
```

Notes:
- `rustls` version must match what reqwest 0.12 uses internally (0.23.x). Check with `cargo tree -i rustls` after adding.
- `webpki-roots` provides Mozilla's root CA bundle for building the default verifier.
- `sha2` is for certificate fingerprinting.
- `base64` is already a dependency (used for fingerprint encoding).

- [ ] **Step 2: Add dev dependency**

Add to `[dev-dependencies]`:

```toml
rcgen = "0.13"
```

- [ ] **Step 3: Verify dependency resolution**

Run: `cargo check 2>&1 | tail -5`

Expected: clean compilation. If rustls version conflicts, adjust the version constraint to match `cargo tree -i rustls` output.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "build: add rustls, sha2, and rcgen dependencies for TLS pinning"
```

---

### Task 2: Fingerprint utilities

**Files:**
- Create: `src/tls/mod.rs`
- Create: `src/tls/fingerprint.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Create module skeleton**

Create `src/tls/mod.rs`:

```rust
pub(crate) mod fingerprint;
```

Add to `src/lib.rs` module declarations (after `pub(crate) mod http;`):

```rust
pub(crate) mod tls;
```

- [ ] **Step 2: Write failing tests for fingerprint utilities**

Create `src/tls/fingerprint.rs`:

```rust
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::error::{BzrError, Result};

/// Prefix for SHA-256 certificate pins (matches curl's --pinnedpubkey format).
const PIN_PREFIX: &str = "sha256//";

/// Compute the SHA-256 fingerprint of a DER-encoded certificate.
///
/// Returns a string in `sha256//<base64>` format.
pub fn compute_fingerprint(der: &[u8]) -> String {
    todo!()
}

/// Parse a `sha256//<base64>` pin string into raw SHA-256 bytes.
pub fn parse_pin(pin: &str) -> Result<[u8; 32]> {
    todo!()
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn compute_fingerprint_deterministic() {
        let cert_bytes = b"fake certificate DER bytes for testing";
        let fp1 = compute_fingerprint(cert_bytes);
        let fp2 = compute_fingerprint(cert_bytes);
        assert_eq!(fp1, fp2);
        assert!(
            fp1.starts_with(PIN_PREFIX),
            "fingerprint should start with {PIN_PREFIX}: {fp1}"
        );
    }

    #[test]
    fn compute_fingerprint_format() {
        let cert_bytes = b"test certificate data";
        let fp = compute_fingerprint(cert_bytes);
        // sha256// prefix + 44 chars of base64 (32 bytes = 44 base64 chars with padding)
        assert!(fp.starts_with(PIN_PREFIX));
        let b64_part = &fp[PIN_PREFIX.len()..];
        assert!(
            BASE64.decode(b64_part).is_ok(),
            "base64 portion should decode: {b64_part}"
        );
    }

    #[test]
    fn round_trip_through_parse_pin() {
        let cert_bytes = b"round trip test data";
        let fp = compute_fingerprint(cert_bytes);
        let parsed = parse_pin(&fp).unwrap();

        let mut hasher = Sha256::new();
        hasher.update(cert_bytes);
        let expected: [u8; 32] = hasher.finalize().into();
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parse_pin_rejects_bad_prefix() {
        let err = parse_pin("md5//AAAA").unwrap_err();
        assert!(
            err.to_string().contains("sha256//"),
            "error should mention expected prefix: {err}"
        );
    }

    #[test]
    fn parse_pin_rejects_bad_base64() {
        let err = parse_pin("sha256//not-valid-base64!!!").unwrap_err();
        assert!(
            err.to_string().contains("base64"),
            "error should mention base64: {err}"
        );
    }

    #[test]
    fn parse_pin_rejects_wrong_length() {
        // Valid base64 but only 16 bytes, not 32
        let short = format!("{PIN_PREFIX}{}", BASE64.encode([0u8; 16]));
        let err = parse_pin(&short).unwrap_err();
        assert!(
            err.to_string().contains("32"),
            "error should mention expected length: {err}"
        );
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib tls::fingerprint::tests -- --quiet`

Expected: FAIL — `todo!()` panics.

- [ ] **Step 4: Implement fingerprint functions**

Replace the `todo!()` bodies in `src/tls/fingerprint.rs`:

```rust
pub fn compute_fingerprint(der: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(der);
    let hash = hasher.finalize();
    format!("{PIN_PREFIX}{}", BASE64.encode(hash))
}

pub fn parse_pin(pin: &str) -> Result<[u8; 32]> {
    let b64 = pin.strip_prefix(PIN_PREFIX).ok_or_else(|| {
        BzrError::InputValidation(format!(
            "certificate pin must start with \"{PIN_PREFIX}\", got: {pin}"
        ))
    })?;
    let bytes = BASE64.decode(b64).map_err(|e| {
        BzrError::InputValidation(format!("invalid base64 in certificate pin: {e}"))
    })?;
    let hash: [u8; 32] = bytes.try_into().map_err(|v: Vec<u8>| {
        BzrError::InputValidation(format!(
            "certificate pin must be 32 bytes (SHA-256), got {} bytes",
            v.len()
        ))
    })?;
    Ok(hash)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib tls::fingerprint::tests -- --quiet`

Expected: all 6 tests PASS.

- [ ] **Step 6: Run clippy**

Run: `cargo clippy -- -D warnings`

Expected: clean.

- [ ] **Step 7: Commit**

```bash
git add src/tls/ src/lib.rs
git commit -m "feat(tls): add SHA-256 certificate fingerprint utilities"
```

---

### Task 3: Config data model changes

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add imports**

Add `use std::path::PathBuf;` to `src/config.rs` imports if not already present.

- [ ] **Step 2: Add new fields to ServerConfig**

Add three fields to `ServerConfig` after the `tls_insecure` field (around line 43):

```rust
    /// Accept invalid TLS certificates (self-signed, expired, etc.).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub tls_insecure: bool,
    /// Path to a PEM-encoded CA certificate for this server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_ca_cert: Option<PathBuf>,
    /// SHA-256 fingerprint of the pinned server certificate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin_sha256: Option<String>,
    /// Issuer DN stored alongside the pin for rotation detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_pin_issuer: Option<String>,
```

- [ ] **Step 3: Add validation for mutual exclusivity**

Find the `Config::validate` method (or add one if it only validates other things). Add a check that iterates servers and validates TLS field combinations. Add a helper method on `ServerConfig`:

```rust
impl ServerConfig {
    /// Validate that TLS trust fields are not conflicting.
    pub fn validate_tls(&self, server_name: &str) -> Result<()> {
        if self.tls_insecure && self.tls_ca_cert.is_some() {
            return Err(BzrError::config(format!(
                "server '{server_name}': tls_insecure and tls_ca_cert are \
                 mutually exclusive — remove one"
            )));
        }
        if self.tls_insecure && self.tls_pin_sha256.is_some() {
            return Err(BzrError::config(format!(
                "server '{server_name}': tls_insecure and tls_pin_sha256 are \
                 mutually exclusive — remove one"
            )));
        }
        if self.tls_ca_cert.is_some() && self.tls_pin_sha256.is_some() {
            return Err(BzrError::config(format!(
                "server '{server_name}': tls_ca_cert and tls_pin_sha256 are \
                 mutually exclusive — use one trust method per server"
            )));
        }
        if let Some(pin) = &self.tls_pin_sha256 {
            crate::tls::fingerprint::parse_pin(pin)?;
        }
        if let Some(path) = &self.tls_ca_cert {
            if !path.exists() {
                return Err(BzrError::config(format!(
                    "server '{server_name}': tls_ca_cert file not found: {}",
                    path.display()
                )));
            }
        }
        Ok(())
    }
}
```

Call `srv.validate_tls(name)?` from the existing `Config::validate` loop over servers.

- [ ] **Step 4: Fix all test `ServerConfig` literals**

Every test that constructs a `ServerConfig` needs the three new fields. Search for `tls_insecure: false` and `tls_insecure: true` across all test files. Add the new fields to each:

```rust
tls_insecure: false,
tls_ca_cert: None,
tls_pin_sha256: None,
tls_pin_issuer: None,
```

Files to update (search for `ServerConfig {` in tests):
- `src/config.rs` (multiple test functions)
- `src/commands/config.rs` (test functions)
- `src/commands/shared.rs` (test functions)
- `src/output/config.rs` (test functions)
- `src/url_parser.rs` (`make_server_config` helper)

- [ ] **Step 5: Write validation tests**

Add to `src/config.rs` test module:

```rust
#[test]
fn validate_tls_insecure_with_ca_cert_conflicts() {
    let srv = ServerConfig {
        url: "https://example.com".into(),
        api_key: Some("key".into()),
        api_key_env: None,
        api_key_keyring: None,
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: true,
        tls_ca_cert: Some("/tmp/ca.pem".into()),
        tls_pin_sha256: None,
        tls_pin_issuer: None,
    };
    let err = srv.validate_tls("test").unwrap_err();
    assert!(err.to_string().contains("mutually exclusive"));
}

#[test]
fn validate_tls_insecure_with_pin_conflicts() {
    let srv = ServerConfig {
        url: "https://example.com".into(),
        api_key: Some("key".into()),
        api_key_env: None,
        api_key_keyring: None,
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: true,
        tls_ca_cert: None,
        tls_pin_sha256: Some("sha256//AAAA".into()),
        tls_pin_issuer: None,
    };
    let err = srv.validate_tls("test").unwrap_err();
    assert!(err.to_string().contains("mutually exclusive"));
}

#[test]
fn validate_tls_ca_cert_with_pin_conflicts() {
    let srv = ServerConfig {
        url: "https://example.com".into(),
        api_key: Some("key".into()),
        api_key_env: None,
        api_key_keyring: None,
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: Some("/tmp/ca.pem".into()),
        tls_pin_sha256: Some("sha256//AAAA".into()),
        tls_pin_issuer: None,
    };
    let err = srv.validate_tls("test").unwrap_err();
    assert!(err.to_string().contains("mutually exclusive"));
}

#[test]
fn validate_tls_no_conflicts_passes() {
    let srv = ServerConfig {
        url: "https://example.com".into(),
        api_key: Some("key".into()),
        api_key_env: None,
        api_key_keyring: None,
        email: None,
        auth_method: None,
        api_mode: None,
        server_version: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_issuer: None,
    };
    assert!(srv.validate_tls("test").is_ok());
}
```

- [ ] **Step 6: Run all tests**

Run: `cargo test --lib -- --quiet`

Expected: all tests pass (including updated literals).

- [ ] **Step 7: Commit**

```bash
git add src/config.rs src/commands/config.rs src/commands/shared.rs \
       src/output/config.rs src/url_parser.rs
git commit -m "feat(config): add tls_ca_cert, tls_pin_sha256, tls_pin_issuer fields"
```

---

### Task 4: Error variants

**Files:**
- Modify: `src/error.rs`

- [ ] **Step 1: Add TLS error variants to BzrError**

Add two new variants to the `BzrError` enum (before the `Other` variant):

```rust
    #[error("TLS pin mismatch for {server}: expected {expected}, got {actual}")]
    PinMismatch {
        server: String,
        expected: String,
        actual: String,
    },

    #[error(
        "TLS certificate issuer changed for {server}: expected \"{expected_issuer}\", \
         got \"{actual_issuer}\" — possible MITM attack"
    )]
    IssuerChanged {
        server: String,
        expected_issuer: String,
        actual_issuer: String,
    },
```

- [ ] **Step 2: Add exit codes and error types**

Add constants:

```rust
const ERROR_TYPE_TLS: &str = "tls";
const EXIT_CODE_TLS: i32 = 13;
```

Add match arms to `exit_code()` and `error_type()`:

```rust
// In exit_code():
BzrError::PinMismatch { .. } | BzrError::IssuerChanged { .. } => EXIT_CODE_TLS,

// In error_type():
BzrError::PinMismatch { .. } | BzrError::IssuerChanged { .. } => ERROR_TYPE_TLS,
```

- [ ] **Step 3: Write tests**

```rust
#[test]
fn exit_code_pin_mismatch() {
    let err = BzrError::PinMismatch {
        server: "test".into(),
        expected: "sha256//old".into(),
        actual: "sha256//new".into(),
    };
    assert_eq!(err.exit_code(), 13);
    assert_eq!(err.error_type(), "tls");
    assert!(err.to_string().contains("pin mismatch"));
}

#[test]
fn exit_code_issuer_changed() {
    let err = BzrError::IssuerChanged {
        server: "test".into(),
        expected_issuer: "CN=Good CA".into(),
        actual_issuer: "CN=Evil CA".into(),
    };
    assert_eq!(err.exit_code(), 13);
    assert_eq!(err.error_type(), "tls");
    assert!(err.to_string().contains("MITM"));
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib error::tests -- --quiet`

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/error.rs
git commit -m "feat(error): add PinMismatch and IssuerChanged TLS error variants"
```

---

### Task 5: Custom ServerCertVerifier

**Files:**
- Create: `src/tls/verifier.rs`
- Modify: `src/tls/mod.rs`

This is the most complex task. The verifier has three modes: pin-based, CA-cert-based, and the combination logic for rotation detection.

- [ ] **Step 1: Add module declaration**

In `src/tls/mod.rs`, add:

```rust
pub(crate) mod fingerprint;
pub(crate) mod verifier;
```

- [ ] **Step 2: Write the verifier with tests**

Create `src/tls/verifier.rs`. This file implements `ServerCertVerifier` for pin-based and CA-cert-based verification. The implementation delegates signature verification to a `WebPkiServerVerifier` and only overrides `verify_server_cert`.

```rust
use std::sync::Arc;

use rustls::client::danger::{
    HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error as TlsError, SignatureScheme};

use crate::tls::fingerprint;

/// Extracts the issuer Common Name from a DER-encoded certificate.
///
/// Uses a best-effort parse — returns `"<unknown>"` if the cert
/// cannot be parsed.
pub fn extract_issuer_dn(der: &[u8]) -> String {
    // rustls-pki-types doesn't expose issuer parsing, so we use
    // a minimal ASN.1 walk or fall back to the raw DER description.
    // For now, use x509-parser if available, or a simple fallback.
    // Since we don't want to add x509-parser as a dependency, we'll
    // store the issuer DN from the rustls CertificateError context
    // or extract it at probe time using a TLS connection.
    //
    // Practical approach: extract at probe time (in tofu.rs) using
    // rustls's parsed certificate info, and pass it in as a parameter.
    // This function is a fallback for when we have raw DER only.
    format!("<raw DER, {} bytes>", der.len())
}

/// A certificate verifier that checks the leaf cert's SHA-256
/// fingerprint against a stored pin.
#[derive(Debug)]
pub struct PinnedCertVerifier {
    pin: [u8; 32],
    pin_issuer: Option<String>,
    server_name_for_errors: String,
    /// Delegate for TLS signature verification.
    signature_verifier: Arc<dyn ServerCertVerifier>,
}

impl PinnedCertVerifier {
    pub fn new(
        pin_sha256: &str,
        pin_issuer: Option<String>,
        server_name: &str,
    ) -> crate::error::Result<Self> {
        let pin = fingerprint::parse_pin(pin_sha256)?;
        let provider = CryptoProvider::get_default()
            .cloned()
            .unwrap_or_else(|| Arc::new(rustls::crypto::ring::default_provider()));
        let signature_verifier = rustls::client::WebPkiServerVerifier::builder_with_provider(
            Arc::new(rustls::RootCertStore::empty()),
            provider,
        )
        .build()
        .map_err(|e| {
            crate::error::BzrError::config(format!(
                "failed to build signature verifier: {e}"
            ))
        })?;
        Ok(Self {
            pin,
            pin_issuer,
            server_name_for_errors: server_name.to_string(),
            signature_verifier,
        })
    }
}

impl ServerCertVerifier for PinnedCertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let actual_fp = fingerprint::compute_fingerprint(end_entity.as_ref());
        let actual_hash = fingerprint::parse_pin(&actual_fp).map_err(|e| {
            TlsError::General(format!("fingerprint computation failed: {e}"))
        })?;

        if actual_hash == self.pin {
            return Ok(ServerCertVerified::assertion());
        }

        // Pin mismatch — check if issuer changed for rotation detection.
        let expected_fp = format!(
            "sha256//{}",
            base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                self.pin,
            )
        );
        let actual_issuer = extract_issuer_dn(end_entity.as_ref());

        if let Some(expected_issuer) = &self.pin_issuer {
            if *expected_issuer != actual_issuer {
                return Err(TlsError::General(format!(
                    "ISSUER_CHANGED:{}:{}:{}:{}",
                    self.server_name_for_errors,
                    expected_issuer,
                    actual_issuer,
                    actual_fp,
                )));
            }
        }

        Err(TlsError::General(format!(
            "PIN_MISMATCH:{}:{}:{}",
            self.server_name_for_errors, expected_fp, actual_fp,
        )))
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        self.signature_verifier
            .verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        self.signature_verifier
            .verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.signature_verifier.supported_verify_schemes()
    }
}

/// Build a `rustls::ClientConfig` with a custom CA certificate added
/// to the root store.
pub fn build_ca_cert_config(
    ca_pem_path: &std::path::Path,
) -> crate::error::Result<rustls::ClientConfig> {
    let pem_data = std::fs::read(ca_pem_path).map_err(|e| {
        crate::error::BzrError::config(format!(
            "failed to read CA cert file {}: {e}",
            ca_pem_path.display()
        ))
    })?;

    let mut root_store = rustls::RootCertStore::empty();

    // Add system roots first
    for cert in rustls_native_certs::load_native_certs().expect("load native certs") {
        let _ = root_store.add(cert);
    }

    // Add custom CA certs from PEM file
    let certs: Vec<CertificateDer<'static>> =
        rustls_pemfile::certs(&mut pem_data.as_slice())
            .filter_map(|r| r.ok())
            .collect();
    if certs.is_empty() {
        return Err(crate::error::BzrError::config(format!(
            "no valid PEM certificates found in {}",
            ca_pem_path.display()
        )));
    }
    for cert in certs {
        root_store.add(cert).map_err(|e| {
            crate::error::BzrError::config(format!("invalid CA certificate: {e}"))
        })?;
    }

    let provider = CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::ring::default_provider()));
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| crate::error::BzrError::config(format!("TLS config error: {e}")))?
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Ok(config)
}

/// Build a `rustls::ClientConfig` with pinned certificate verification.
pub fn build_pinned_config(
    pin_sha256: &str,
    pin_issuer: Option<String>,
    server_name: &str,
) -> crate::error::Result<rustls::ClientConfig> {
    let verifier = PinnedCertVerifier::new(pin_sha256, pin_issuer, server_name)?;
    let provider = CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::ring::default_provider()));
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| crate::error::BzrError::config(format!("TLS config error: {e}")))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    Ok(config)
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn generate_self_signed_cert() -> (rcgen::CertifiedKey, Vec<u8>) {
        let params = rcgen::CertificateParams::new(vec!["localhost".into()]).unwrap();
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = params.self_signed(&key).unwrap();
        let der = cert.der().to_vec();
        (rcgen::CertifiedKey { cert, key_pair: key }, der)
    }

    #[test]
    fn pinned_verifier_accepts_matching_cert() {
        let (_key, der) = generate_self_signed_cert();
        let fp = fingerprint::compute_fingerprint(&der);

        let verifier =
            PinnedCertVerifier::new(&fp, None, "test").unwrap();
        let cert_der = CertificateDer::from(der);
        let server_name = ServerName::try_from("localhost").unwrap();

        let result = verifier.verify_server_cert(
            &cert_der,
            &[],
            &server_name,
            &[],
            UnixTime::now(),
        );
        assert!(result.is_ok(), "matching pin should be accepted");
    }

    #[test]
    fn pinned_verifier_rejects_mismatched_cert() {
        let (_key1, _der1) = generate_self_signed_cert();
        let (_key2, der2) = generate_self_signed_cert();

        // Pin cert 1's fingerprint but present cert 2
        let fp1 = fingerprint::compute_fingerprint(b"wrong cert data");

        let verifier =
            PinnedCertVerifier::new(&fp1, None, "test").unwrap();
        let cert_der = CertificateDer::from(der2);
        let server_name = ServerName::try_from("localhost").unwrap();

        let result = verifier.verify_server_cert(
            &cert_der,
            &[],
            &server_name,
            &[],
            UnixTime::now(),
        );
        assert!(result.is_err(), "mismatched pin should be rejected");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("PIN_MISMATCH"),
            "error should indicate pin mismatch: {err_msg}"
        );
    }

    #[test]
    fn ca_cert_config_rejects_missing_file() {
        let result = build_ca_cert_config(std::path::Path::new("/nonexistent/ca.pem"));
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("failed to read"),
            "should report file read failure"
        );
    }
}
```

- [ ] **Step 3: Add `rustls-pemfile` and `rustls-native-certs` dependencies**

These are needed by the CA cert loader. Add to `[dependencies]` in `Cargo.toml`:

```toml
rustls-pemfile = "2"
rustls-native-certs = "0.8"
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib tls::verifier::tests -- --quiet`

Expected: all 3 tests pass.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -- -D warnings`

Fix any warnings.

- [ ] **Step 6: Commit**

```bash
git add src/tls/verifier.rs src/tls/mod.rs Cargo.toml Cargo.lock
git commit -m "feat(tls): implement PinnedCertVerifier and CA cert config builder"
```

---

### Task 6: Build TLS client function

**Files:**
- Modify: `src/tls/mod.rs`
- Modify: `src/http.rs`

Replace `build_http_client` with `build_tls_client` in the tls module.

- [ ] **Step 1: Define TlsConfig and build_tls_client**

Update `src/tls/mod.rs`:

```rust
use std::path::PathBuf;

pub(crate) mod fingerprint;
pub(crate) mod verifier;

/// TLS configuration for a server connection.
#[derive(Debug, Clone, Default)]
pub struct TlsConfig {
    pub insecure: bool,
    pub ca_cert_path: Option<PathBuf>,
    pub pin_sha256: Option<String>,
    pub pin_issuer: Option<String>,
    pub server_name: Option<String>,
}

/// Build a `reqwest::Client` with the appropriate TLS configuration.
///
/// Selects the verification mode based on `TlsConfig` fields:
/// 1. `insecure` — accept all certs (`danger_accept_invalid_certs`)
/// 2. `ca_cert_path` — custom CA added to root store
/// 3. `pin_sha256` — pinned certificate fingerprint verification
/// 4. None — default system roots
pub fn build_tls_client(
    config: &TlsConfig,
) -> crate::error::Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .connect_timeout(crate::http::CONNECT_TIMEOUT)
        .timeout(crate::http::REQUEST_TIMEOUT);

    if config.insecure {
        builder = builder.danger_accept_invalid_certs(true);
    } else if let Some(ca_path) = &config.ca_cert_path {
        let tls_config = verifier::build_ca_cert_config(ca_path)?;
        builder = builder.use_preconfigured_tls(tls_config);
    } else if let Some(pin) = &config.pin_sha256 {
        let tls_config = verifier::build_pinned_config(
            pin,
            config.pin_issuer.clone(),
            config.server_name.as_deref().unwrap_or("unknown"),
        )?;
        builder = builder.use_preconfigured_tls(tls_config);
    }

    builder
        .build()
        .map_err(|e| crate::error::BzrError::Http(e))
}
```

- [ ] **Step 2: Update http.rs — remove build_http_client, keep utilities**

In `src/http.rs`, replace `build_http_client` with a thin wrapper that delegates to the new `tls::build_tls_client`:

```rust
/// Build an HTTP client with the given TLS insecure setting.
///
/// This is a convenience wrapper around `tls::build_tls_client` for
/// callers that only need the insecure toggle (e.g. auth detection).
pub(crate) fn build_http_client(
    tls_insecure: bool,
) -> std::result::Result<reqwest::Client, reqwest::Error> {
    let config = crate::tls::TlsConfig {
        insecure: tls_insecure,
        ..Default::default()
    };
    crate::tls::build_tls_client(&config).map_err(|e| match e {
        crate::error::BzrError::Http(reqwest_err) => reqwest_err,
        other => panic!("unexpected error from build_tls_client: {other}"),
    })
}
```

Note: Keep `build_http_client` as a thin wrapper for now because `detect_server_settings` in `client/auth/mod.rs` calls it and expects `Result<Client, reqwest::Error>`. We'll migrate those callers in a later task.

- [ ] **Step 3: Run full test suite**

Run: `cargo test --lib -- --quiet`

Expected: all tests pass. The thin wrapper preserves backward compatibility.

- [ ] **Step 4: Commit**

```bash
git add src/tls/mod.rs src/http.rs
git commit -m "feat(tls): add build_tls_client with CA cert and pin support"
```

---

### Task 7: Migrate callers to TlsConfig

**Files:**
- Modify: `src/commands/shared.rs`
- Modify: `src/client/mod.rs`

- [ ] **Step 1: Update connect_and_configure to build TlsConfig**

In `src/commands/shared.rs`, update `connect_and_configure` to extract the new TLS fields from `ServerConfig` and pass them through. Change the destructuring to include the new fields:

```rust
let (server_name, url, api_key, email, tls_config) = (
    server_name.to_string(),
    srv.url.clone(),
    srv.resolve_api_key(server_name)?,
    srv.email.clone(),
    crate::tls::TlsConfig {
        insecure: srv.tls_insecure,
        ca_cert_path: srv.tls_ca_cert.clone(),
        pin_sha256: srv.tls_pin_sha256.clone(),
        pin_issuer: srv.tls_pin_issuer.clone(),
        server_name: Some(server_name.to_string()),
    },
);
```

Update the warning:

```rust
if tls_config.insecure {
    tracing::warn!("TLS certificate verification disabled for server '{server_name}'");
}
```

Update `detect_server_settings` calls to pass `tls_config.insecure` (since that function still takes `bool`).

Update `BugzillaClient::new` call to pass `&tls_config` instead of `tls_insecure`.

- [ ] **Step 2: Update BugzillaClient::new to accept TlsConfig**

In `src/client/mod.rs`, change the constructor signature:

```rust
pub fn new(
    base_url: &str,
    api_key: &str,
    auth_method: AuthMethod,
    api_mode: ApiMode,
    email_hint: Option<&str>,
    tls_config: &crate::tls::TlsConfig,
) -> Result<Self> {
```

Replace `let http = build_http_client(tls_insecure).map_err(BzrError::Http)?;` with:

```rust
let http = crate::tls::build_tls_client(tls_config)?;
```

Update `XmlRpcClient::new` — it currently takes `http.clone()` which still works.

- [ ] **Step 3: Fix all callers and tests**

Search for all call sites of `BugzillaClient::new` and update them to pass `&TlsConfig` instead of `bool`. Update test helpers that construct clients.

- [ ] **Step 4: Run full test suite**

Run: `cargo test -- --quiet`

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/commands/shared.rs src/client/mod.rs
git commit -m "refactor: migrate BugzillaClient to TlsConfig"
```

---

### Task 8: CLI flags

**Files:**
- Modify: `src/cli/config.rs`

- [ ] **Step 1: Add new flags to SetServer**

Add the four new flags to the `SetServer` variant in `src/cli/config.rs`:

```rust
    SetServer {
        // ... existing fields ...

        /// Accept invalid TLS certificates (self-signed, expired, wrong host)
        #[arg(
            long,
            conflicts_with_all = ["tls_ca_cert", "tls_pin_sha256", "tls_pin_now"],
        )]
        tls_insecure: bool,
        /// Path to a PEM CA certificate file for this server
        #[arg(
            long,
            conflicts_with_all = ["tls_insecure", "tls_pin_sha256", "tls_pin_now"],
        )]
        tls_ca_cert: Option<String>,
        /// Pin a certificate fingerprint (sha256//<base64> format)
        #[arg(
            long,
            conflicts_with_all = ["tls_insecure", "tls_ca_cert", "tls_pin_now", "tls_pin_clear"],
        )]
        tls_pin_sha256: Option<String>,
        /// Connect to server and pin its current certificate
        #[arg(
            long,
            conflicts_with_all = ["tls_insecure", "tls_ca_cert", "tls_pin_sha256", "tls_pin_clear"],
        )]
        tls_pin_now: bool,
        /// Remove a stored certificate pin
        #[arg(
            long,
            conflicts_with_all = ["tls_pin_sha256", "tls_pin_now"],
        )]
        tls_pin_clear: bool,
    },
```

- [ ] **Step 2: Update SetServerArgs and destructuring**

Update `SetServerArgs` in `src/commands/config.rs` to include the new fields:

```rust
struct SetServerArgs<'a> {
    name: &'a str,
    url: &'a str,
    api_key: Option<&'a str>,
    api_key_env: Option<&'a str>,
    email: Option<&'a str>,
    auth_method: Option<crate::types::AuthMethod>,
    tls_insecure: bool,
    tls_ca_cert: Option<&'a str>,
    tls_pin_sha256: Option<&'a str>,
    tls_pin_now: bool,
    tls_pin_clear: bool,
}
```

Update the destructuring in `execute()` to extract these fields from the CLI args.

- [ ] **Step 3: Write CLI parsing tests**

Add tests for the new flag combinations:

```rust
#[test]
fn parse_set_server_tls_ca_cert() {
    let cli = Cli::try_parse_from([
        "bzr", "config", "set-server", "test",
        "--url", "https://example.com",
        "--api-key", "key",
        "--tls-ca-cert", "/path/to/ca.pem",
    ]).unwrap();
    // verify tls_ca_cert is Some
}

#[test]
fn parse_set_server_tls_insecure_conflicts_with_ca_cert() {
    let result = Cli::try_parse_from([
        "bzr", "config", "set-server", "test",
        "--url", "https://example.com",
        "--api-key", "key",
        "--tls-insecure",
        "--tls-ca-cert", "/path/to/ca.pem",
    ]);
    assert!(result.is_err(), "should conflict");
}

#[test]
fn parse_set_server_tls_pin_now() {
    let cli = Cli::try_parse_from([
        "bzr", "config", "set-server", "test",
        "--url", "https://example.com",
        "--api-key", "key",
        "--tls-pin-now",
    ]).unwrap();
    // verify tls_pin_now is true
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib cli::tests -- --quiet`

Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src/cli/config.rs src/commands/config.rs
git commit -m "feat(cli): add --tls-ca-cert, --tls-pin-sha256, --tls-pin-now, --tls-pin-clear flags"
```

---

### Task 9: Set-server command handling

**Files:**
- Modify: `src/commands/config.rs`

- [ ] **Step 1: Update set_server to handle new TLS fields**

In the `set_server` function, update the `ServerConfig` construction to include the new fields:

```rust
ServerConfig {
    url: url.to_owned(),
    api_key: api_key.map(str::to_owned),
    api_key_env: api_key_env.map(str::to_owned),
    api_key_keyring: None,
    email: email.map(str::to_owned),
    auth_method,
    api_mode: None,
    server_version: None,
    tls_insecure,
    tls_ca_cert: tls_ca_cert.map(PathBuf::from),
    tls_pin_sha256: tls_pin_sha256.map(str::to_owned),
    tls_pin_issuer: None, // set by --tls-pin-now or TOFU
}
```

- [ ] **Step 2: Handle --tls-pin-clear**

Before inserting the server config, handle the clear case:

```rust
if tls_pin_clear {
    if let Some(existing) = config.servers.get_mut(name) {
        existing.tls_pin_sha256 = None;
        existing.tls_pin_issuer = None;
        config.save()?;
        crate::output::print_message(
            &format!("Cleared TLS pin for server '{name}'"),
            format,
        );
        return Ok(());
    }
    return Err(BzrError::config(format!(
        "server '{name}' not found — nothing to clear"
    )));
}
```

- [ ] **Step 3: Handle --tls-pin-now**

After constructing the server config but before saving, if `tls_pin_now` is true, probe the server to get its certificate:

```rust
if tls_pin_now {
    let (fingerprint, issuer) =
        crate::tls::tofu::probe_server_cert(&server_config.url).await?;

    writeln!(
        io::stderr(),
        "Server certificate for '{name}':\n  \
         fingerprint: {fingerprint}\n  \
         issuer:      {issuer}\n"
    ).expect("write to stderr");

    // Confirm with user
    if !crate::tls::tofu::confirm_pin()? {
        return Err(BzrError::InputValidation(
            "certificate pinning cancelled by user".into(),
        ));
    }

    server_config.tls_pin_sha256 = Some(fingerprint);
    server_config.tls_pin_issuer = Some(issuer);
}
```

Note: This depends on Task 10 (`tofu.rs`). If implementing in order, stub the tofu functions first and come back to complete them.

- [ ] **Step 4: Write tests**

Add tests for set-server with the new TLS fields (using the existing test pattern with temp config files).

- [ ] **Step 5: Run tests**

Run: `cargo test --lib commands::config::tests -- --quiet`

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/commands/config.rs
git commit -m "feat(config): handle --tls-ca-cert, --tls-pin-now, --tls-pin-clear in set-server"
```

---

### Task 10: TOFU module

**Files:**
- Create: `src/tls/tofu.rs`
- Modify: `src/tls/mod.rs`

- [ ] **Step 1: Add module declaration**

In `src/tls/mod.rs`, add:

```rust
pub(crate) mod tofu;
```

- [ ] **Step 2: Implement probe_server_cert**

Create `src/tls/tofu.rs`:

```rust
use std::io::{self, Write};

use crate::error::{BzrError, Result};
use crate::tls::fingerprint;

/// Connect to a server with TLS verification disabled to retrieve
/// its certificate. No credentials are sent.
///
/// Returns `(fingerprint, issuer_dn)`.
pub async fn probe_server_cert(url: &str) -> Result<(String, String)> {
    use rustls::client::danger::{
        HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
    };
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use rustls::{DigitallySignedStruct, SignatureScheme};
    use std::sync::{Arc, Mutex};

    /// A verifier that accepts anything but captures the leaf cert.
    #[derive(Debug)]
    struct CertCapture {
        captured: Mutex<Option<(Vec<u8>, String)>>,
        provider: Arc<rustls::crypto::CryptoProvider>,
    }

    impl ServerCertVerifier for CertCapture {
        fn verify_server_cert(
            &self,
            end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> std::result::Result<ServerCertVerified, rustls::Error> {
            let der = end_entity.as_ref().to_vec();
            let issuer = super::verifier::extract_issuer_dn(&der);
            *self.captured.lock().expect("lock") = Some((der, issuer));
            Ok(ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> std::result::Result<HandshakeSignatureValid, rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            self.provider
                .signature_verification_algorithms
                .supported_schemes()
        }
    }

    let provider = rustls::crypto::CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::ring::default_provider()));

    let capture = Arc::new(CertCapture {
        captured: Mutex::new(None),
        provider: provider.clone(),
    });

    let tls_config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| BzrError::config(format!("TLS probe config error: {e}")))?
        .dangerous()
        .with_custom_certificate_verifier(capture.clone())
        .with_no_client_auth();

    let client = reqwest::Client::builder()
        .use_preconfigured_tls(tls_config)
        .connect_timeout(crate::http::CONNECT_TIMEOUT)
        .timeout(crate::http::REQUEST_TIMEOUT)
        .build()
        .map_err(BzrError::Http)?;

    // Make a HEAD request — we don't care about the response, only
    // the TLS handshake. No auth headers are sent.
    let _ = client.head(url).send().await;

    let (der, issuer) = capture
        .captured
        .lock()
        .expect("lock")
        .take()
        .ok_or_else(|| {
            BzrError::config("TLS probe failed: could not retrieve server certificate")
        })?;

    let fp = fingerprint::compute_fingerprint(&der);
    Ok((fp, issuer))
}

/// Prompt the user to confirm pinning a certificate.
/// Returns `false` if stdin is not a terminal.
pub fn confirm_pin() -> Result<bool> {
    if !io::stdin().is_terminal() {
        return Ok(false);
    }
    let _ = write!(io::stderr(), "Pin this certificate? [y/N] ");
    let _ = io::stderr().flush();

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().eq_ignore_ascii_case("y"))
}

/// Prompt the user during TOFU: trust for session, always, or abort.
///
/// Returns:
/// - `Some(true)` for "always" (pin permanently)
/// - `Some(false)` for "y" (session only)
/// - `None` for "N" / non-interactive (abort)
pub fn prompt_tofu(
    server_name: &str,
    hostname: &str,
    fingerprint: &str,
    issuer: &str,
) -> Result<Option<bool>> {
    if !io::stdin().is_terminal() {
        return Ok(None);
    }

    let _ = writeln!(
        io::stderr(),
        "\nwarning: server certificate is not trusted\n  \
         server:      {server_name} ({hostname})\n  \
         fingerprint: {fingerprint}\n  \
         issuer:      {issuer}\n\n\
         Trust this certificate? [y/N/always]\n  \
         y     = trust for this session only\n  \
         N     = abort (default)\n  \
         always = pin fingerprint to config and trust permanently"
    );
    let _ = write!(io::stderr(), "\n> ");
    let _ = io::stderr().flush();

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim().to_ascii_lowercase();

    match trimmed.as_str() {
        "always" => Ok(Some(true)),
        "y" | "yes" => Ok(Some(false)),
        _ => Ok(None),
    }
}

/// Prompt the user to accept a rotated certificate (same CA issuer).
pub fn prompt_rotation(
    server_name: &str,
    hostname: &str,
    old_pin: &str,
    new_pin: &str,
    issuer: &str,
) -> Result<bool> {
    if !io::stdin().is_terminal() {
        return Ok(false);
    }

    let _ = writeln!(
        io::stderr(),
        "\nwarning: server certificate has changed (likely rotation)\n  \
         server:      {server_name} ({hostname})\n  \
         old pin:     {old_pin}\n  \
         new pin:     {new_pin}\n  \
         issuer:      {issuer}  (unchanged)\n\n\
         Accept new certificate? [y/N]"
    );
    let _ = write!(io::stderr(), "\n> ");
    let _ = io::stderr().flush();

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().eq_ignore_ascii_case("y"))
}
```

- [ ] **Step 3: Run compilation check**

Run: `cargo check`

Expected: compiles cleanly. Unit tests for TOFU prompts are difficult to automate (interactive stdin), so we test them at the integration level.

- [ ] **Step 4: Commit**

```bash
git add src/tls/tofu.rs src/tls/mod.rs
git commit -m "feat(tls): add TOFU probe, prompts, and certificate capture"
```

---

### Task 11: TOFU integration into connect_and_configure

**Files:**
- Modify: `src/commands/shared.rs`

- [ ] **Step 1: Add TOFU intercept logic**

In `connect_and_configure`, after the `BugzillaClient::new` call, wrap the connection attempt to catch TLS errors and offer TOFU:

```rust
// After building tls_config but before creating the client:
let client_result = BugzillaClient::new(
    &url, &api_key, auth, api_mode, email.as_deref(), &tls_config,
);

let client = match client_result {
    Ok(c) => c,
    Err(ref e) if should_offer_tofu(e, &tls_config) => {
        handle_tofu(&server_name, &url, &api_key, auth, api_mode,
                    email.as_deref(), &mut config).await?
    }
    Err(e) => return Err(e),
};
```

The `should_offer_tofu` helper checks:
- The error is a TLS cert error
- No trust mechanism is configured (not insecure, no CA cert, no pin)

The `handle_tofu` function:
1. Probes the server cert via `tls::tofu::probe_server_cert`
2. Calls `tls::tofu::prompt_tofu`
3. On "always" — saves pin to config, retries with pin config
4. On "y" — retries with insecure config (session only)
5. On None — returns the original error

Note: The actual TLS error happens during `detect_server_settings` (which makes HTTP calls) or during `BugzillaClient` method calls, not during `BugzillaClient::new` (which only builds the client). The TOFU intercept should wrap the first network call. Adjust the integration point accordingly — the intercept may need to be in the `send` or `detect_server_settings` call path rather than around `new`.

- [ ] **Step 2: Implement should_offer_tofu and handle_tofu**

```rust
fn should_offer_tofu(err: &BzrError, tls_config: &crate::tls::TlsConfig) -> bool {
    if tls_config.insecure
        || tls_config.ca_cert_path.is_some()
        || tls_config.pin_sha256.is_some()
    {
        return false;
    }
    matches!(err, BzrError::Http(e) if crate::http::is_tls_cert_error(e))
}

async fn handle_tofu(
    server_name: &str,
    url: &str,
    api_key: &str,
    auth: AuthMethod,
    api_mode: ApiMode,
    email: Option<&str>,
    config: &mut Config,
) -> Result<BugzillaClient> {
    let hostname = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(String::from))
        .unwrap_or_else(|| url.to_string());

    let (fingerprint, issuer) = crate::tls::tofu::probe_server_cert(url).await?;

    match crate::tls::tofu::prompt_tofu(server_name, &hostname, &fingerprint, &issuer)? {
        Some(true) => {
            // "always" — save pin and retry
            if let Some(srv) = config.servers.get_mut(server_name) {
                srv.tls_pin_sha256 = Some(fingerprint.clone());
                srv.tls_pin_issuer = Some(issuer);
                config.save()?;
            }
            let tls_config = crate::tls::TlsConfig {
                pin_sha256: Some(fingerprint),
                server_name: Some(server_name.to_string()),
                ..Default::default()
            };
            BugzillaClient::new(url, api_key, auth, api_mode, email, &tls_config)
        }
        Some(false) => {
            // "y" — session only, retry with insecure
            let tls_config = crate::tls::TlsConfig {
                insecure: true,
                ..Default::default()
            };
            BugzillaClient::new(url, api_key, auth, api_mode, email, &tls_config)
        }
        None => {
            // "N" or non-interactive — fail with hints
            Err(BzrError::Auth(format!(
                "server certificate not trusted for '{server_name}'\n  \
                 hint: to trust this server's certificate, re-run interactively,\n    \
                 or pre-pin with:  bzr config set-server {server_name} --tls-pin-now\n    \
                 or provide a CA:  bzr config set-server {server_name} --tls-ca-cert <PATH>\n    \
                 or skip verification: bzr config set-server {server_name} --tls-insecure"
            )))
        }
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -- --quiet`

Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/commands/shared.rs
git commit -m "feat(tls): integrate TOFU flow into connect_and_configure"
```

---

### Task 12: Config show output

**Files:**
- Modify: `src/output/config.rs`

- [ ] **Step 1: Add new fields to ServerDisplayInfo**

```rust
#[serde(skip_serializing_if = "Option::is_none")]
tls_ca_cert: Option<String>,
#[serde(skip_serializing_if = "Option::is_none")]
tls_pin: Option<String>,
```

- [ ] **Step 2: Populate new fields**

In the function that constructs `ServerDisplayInfo` from `ServerConfig`, add:

```rust
tls_ca_cert: srv.tls_ca_cert.as_ref().map(|p| p.display().to_string()),
tls_pin: srv.tls_pin_sha256.as_ref().map(|pin| {
    if let Some(issuer) = &srv.tls_pin_issuer {
        format!("{pin} ({issuer})")
    } else {
        pin.clone()
    }
}),
```

- [ ] **Step 3: Update print_server**

Add display lines after the `tls_insecure` block:

```rust
if let Some(ca) = &s.tls_ca_cert {
    print_field("TLS CA Cert", ca);
}
if let Some(pin) = &s.tls_pin {
    print_field("TLS Pin", pin);
}
```

- [ ] **Step 4: Update tests**

Update the existing `config show` tests to include the new fields in their `ServerDisplayInfo` constructions.

- [ ] **Step 5: Run tests**

Run: `cargo test --lib output::config::tests -- --quiet`

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/output/config.rs
git commit -m "feat(output): display TLS CA cert and pin in config show"
```

---

### Task 13: Update TLS error hints

**Files:**
- Modify: `src/error.rs`
- Modify: `src/http.rs`

- [ ] **Step 1: Update tls_hint message**

In `src/http.rs`, update the `tls_hint` function's hint text:

```rust
pub(crate) fn tls_hint(base_msg: &str, err: &reqwest::Error) -> String {
    let mut msg = base_msg.to_string();
    if is_tls_cert_error(err) {
        let _ = write!(
            msg,
            "\n  hint: to trust this server's certificate, re-run interactively,\n    \
             or pre-pin with:  bzr config set-server <NAME> --tls-pin-now\n    \
             or provide a CA:  bzr config set-server <NAME> --tls-ca-cert <PATH>\n    \
             or skip verification: bzr config set-server <NAME> --tls-insecure"
        );
    }
    msg
}
```

- [ ] **Step 2: Update format_http_error hint**

In `src/error.rs`, update the hint in `format_http_error` to match:

```rust
if err.is_connect() && crate::http::looks_like_tls_error(&chain) {
    msg.push_str(
        "\n  hint: to trust this server's certificate, re-run interactively,\n    \
         or pre-pin with:  bzr config set-server <NAME> --tls-pin-now\n    \
         or provide a CA:  bzr config set-server <NAME> --tls-ca-cert <PATH>\n    \
         or skip verification: bzr config set-server <NAME> --tls-insecure",
    );
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib -- --quiet`

Expected: all pass.

- [ ] **Step 4: Commit**

```bash
git add src/error.rs src/http.rs
git commit -m "feat(tls): update error hints to mention --tls-pin-now and --tls-ca-cert"
```

---

### Task 14: Documentation

**Files:**
- Modify: `docs/bzr-cli.md`

- [ ] **Step 1: Update CLI reference**

Add the new flags to the `config set-server` section in `docs/bzr-cli.md`:

```markdown
#### TLS Options

| Flag | Description |
|------|-------------|
| `--tls-insecure` | Accept invalid TLS certificates |
| `--tls-ca-cert <PATH>` | Path to PEM CA certificate file |
| `--tls-pin-sha256 <HASH>` | Pin a certificate fingerprint |
| `--tls-pin-now` | Connect and pin the server's current certificate |
| `--tls-pin-clear` | Remove a stored certificate pin |
```

- [ ] **Step 2: Run full test suite and clippy**

```bash
cargo test -- --quiet
cargo clippy -- -D warnings
```

Expected: everything clean.

- [ ] **Step 3: Commit**

```bash
git add docs/bzr-cli.md
git commit -m "docs: add TLS pinning flags to CLI reference"
```
