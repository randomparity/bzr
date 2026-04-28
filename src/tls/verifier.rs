use std::path::Path;
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, Error as TlsError, RootCertStore, SignatureScheme};
use sha2::{Digest, Sha256};

use base64::Engine;

use crate::error::{BzrError, Result};
use crate::tls::fingerprint::{compute_fingerprint, parse_pin};

/// A rustls `ServerCertVerifier` that validates the leaf certificate's
/// SHA-256 fingerprint against a pinned value, bypassing CA chain
/// verification entirely.
#[derive(Debug)]
pub(crate) struct PinnedCertVerifier {
    /// The expected SHA-256 hash of the leaf certificate DER bytes.
    pin_hash: [u8; 32],
    /// The full `sha256//<base64>` pin string, kept for error messages.
    pin_str: String,
    /// Optional expected issuer DN string for change detection
    /// (legacy fallback for pins created before DER comparison).
    pin_issuer: Option<String>,
    /// Raw DER bytes of the expected issuer SEQUENCE for tamper-proof
    /// comparison. Takes precedence over `pin_issuer` string.
    pin_issuer_der: Option<Vec<u8>>,
    /// The server name this verifier was created for (for errors).
    server_name: String,
    /// Delegate for cryptographic signature verification.
    sig_verifier: Arc<dyn ServerCertVerifier>,
}

impl PinnedCertVerifier {
    /// Build a pinned certificate verifier.
    ///
    /// `pin_sha256` must be in `sha256//<base64>` format.
    /// `pin_issuer_der_b64` is the base64-encoded raw DER of the issuer
    /// SEQUENCE, used for tamper-proof issuer comparison. Falls back to
    /// `pin_issuer` string comparison when `None` (backward compat).
    pub(crate) fn new(
        pin_sha256: &str,
        pin_issuer: Option<String>,
        pin_issuer_der_b64: Option<&str>,
        server_name: &str,
    ) -> Result<Self> {
        let pin_hash = parse_pin(pin_sha256)?;
        let provider = super::default_provider();

        let pin_issuer_der = pin_issuer_der_b64
            .map(|b64| {
                base64::engine::general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|e| {
                        BzrError::config(format!("invalid base64 in tls_pin_issuer_der: {e}"))
                    })
            })
            .transpose()?;

        let mut root_store = RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let sig_verifier = rustls::client::WebPkiServerVerifier::builder_with_provider(
            Arc::new(root_store),
            provider,
        )
        .build()
        .map_err(|e| BzrError::config(format!("failed to build signature verifier: {e}")))?;

        Ok(Self {
            pin_hash,
            pin_str: pin_sha256.to_owned(),
            pin_issuer,
            pin_issuer_der,
            server_name: server_name.to_owned(),
            sig_verifier,
        })
    }

    /// Check whether the leaf certificate's issuer matches the pinned issuer.
    /// Returns `Err(TlsError::General(..))` with an `ISSUER_CHANGED` message
    /// when the issuer differs; `Ok(())` when no issuer pin is configured or
    /// the pinned issuer matches.
    ///
    /// Prefers raw DER comparison (tamper-proof). Falls back to string
    /// comparison for legacy pins created before DER storage was added.
    fn check_issuer_change(&self, leaf_der: &[u8]) -> std::result::Result<(), TlsError> {
        if let Some(expected_der) = &self.pin_issuer_der {
            if let Some(actual_der) = extract_issuer_der(leaf_der) {
                if *expected_der != actual_der {
                    return Err(TlsError::General(format!(
                        "ISSUER_CHANGED for {}: issuer DER mismatch \
                         (expected {} bytes, got {} bytes)",
                        self.server_name,
                        expected_der.len(),
                        actual_der.len()
                    )));
                }
            }
        } else if let Some(expected_issuer) = &self.pin_issuer {
            let actual_issuer = extract_issuer_dn(leaf_der);
            if actual_issuer != *expected_issuer {
                return Err(TlsError::General(format!(
                    "ISSUER_CHANGED for {}: expected \"{}\", \
                     got \"{}\"",
                    self.server_name, expected_issuer, actual_issuer
                )));
            }
        }
        Ok(())
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
    ) -> std::result::Result<ServerCertVerified, TlsError> {
        let actual_hash: [u8; 32] = Sha256::digest(end_entity.as_ref()).into();

        if actual_hash == self.pin_hash {
            return Ok(ServerCertVerified::assertion());
        }

        self.check_issuer_change(end_entity.as_ref())?;

        let actual_fp = compute_fingerprint(end_entity.as_ref());
        let actual_issuer = extract_issuer_dn(end_entity.as_ref());
        Err(TlsError::General(format!(
            "PIN_MISMATCH for {}: expected {}, got {}, issuer {}",
            self.server_name, self.pin_str, actual_fp, actual_issuer
        )))
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, TlsError> {
        self.sig_verifier.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<HandshakeSignatureValid, TlsError> {
        self.sig_verifier.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.sig_verifier.supported_verify_schemes()
    }
}

/// Build a `rustls::ClientConfig` that trusts system roots plus any
/// additional CA certificates from a PEM file on disk.
pub(crate) fn build_ca_cert_config(ca_pem_path: &Path) -> Result<rustls::ClientConfig> {
    let pem_data = std::fs::read(ca_pem_path).map_err(|e| {
        BzrError::config(format!(
            "failed to read CA certificate file {}: {e}",
            ca_pem_path.display()
        ))
    })?;

    let mut root_store = RootCertStore::empty();

    // Add system roots.
    let native_certs = rustls_native_certs::load_native_certs();
    for cert in native_certs.certs {
        let _ = root_store.add(cert);
    }

    // Parse and add custom CA certs from the PEM file.
    let custom_certs: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(&pem_data)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| {
            BzrError::config(format!(
                "failed to parse PEM certificates from {}: {e}",
                ca_pem_path.display()
            ))
        })?;

    if custom_certs.is_empty() {
        return Err(BzrError::config(format!(
            "no valid PEM certificates found in {}",
            ca_pem_path.display()
        )));
    }

    for cert in custom_certs {
        root_store.add(cert).map_err(|e| {
            BzrError::config(format!(
                "failed to add CA certificate from {}: {e}",
                ca_pem_path.display()
            ))
        })?;
    }

    let config = super::base_tls_builder("protocol versions")?
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(config)
}

/// Build a `rustls::ClientConfig` that uses a `PinnedCertVerifier`
/// for certificate pinning instead of CA chain validation.
pub(crate) fn build_pinned_config(
    pin_sha256: &str,
    pin_issuer: Option<String>,
    pin_issuer_der: Option<&str>,
    server_name: &str,
) -> Result<rustls::ClientConfig> {
    let verifier = PinnedCertVerifier::new(pin_sha256, pin_issuer, pin_issuer_der, server_name)?;

    let config = super::base_tls_builder("protocol versions")?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();

    Ok(config)
}

/// Navigate to the start of the issuer field within a DER-encoded
/// X.509 certificate, returning the remaining bytes starting at the
/// issuer SEQUENCE. Shared by both `extract_issuer_der` and
/// `extract_issuer_dn`.
fn navigate_to_issuer(cert_der: &[u8]) -> Option<&[u8]> {
    let (_, content) = parse_der_sequence(cert_der)?;
    let (_, tbs) = parse_der_sequence(content)?;
    let mut pos = tbs;
    // Skip optional version [0] EXPLICIT
    if pos.first()? & 0xe0 == 0xa0 {
        let (rest, _) = skip_der_element(pos)?;
        pos = rest;
    }
    // Skip serialNumber INTEGER
    let (rest, _) = skip_der_element(pos)?;
    pos = rest;
    // Skip signature AlgorithmIdentifier SEQUENCE
    let (rest, _) = skip_der_element(pos)?;
    Some(rest)
}

/// Extract the raw DER bytes of the issuer SEQUENCE (tag + length +
/// content) from a DER-encoded X.509 certificate.
pub(crate) fn extract_issuer_der(cert_der: &[u8]) -> Option<Vec<u8>> {
    let pos = navigate_to_issuer(cert_der)?;
    let (rest_after_issuer, _) = skip_der_element(pos)?;
    let issuer_len = pos.len() - rest_after_issuer.len();
    Some(pos[..issuer_len].to_vec())
}

/// Best-effort extraction of issuer information from DER-encoded
/// certificate bytes. Returns a fallback string if parsing fails.
pub(crate) fn extract_issuer_dn(der: &[u8]) -> String {
    // X.509 DER structure (simplified):
    // SEQUENCE {
    //   SEQUENCE {                    -- TBSCertificate
    //     [0] EXPLICIT version       -- optional
    //     INTEGER serialNumber
    //     SEQUENCE signature
    //     SEQUENCE issuer            -- what we want
    //     ...
    //   }
    //   ...
    // }
    //
    // This is a best-effort parser that walks the outer SEQUENCE,
    // the TBSCertificate SEQUENCE, skips version/serial/signature,
    // and returns the raw bytes of the issuer field as a hex string.
    // A proper ASN.1 parser will replace this later.
    parse_issuer_from_tbs(der).unwrap_or_else(|| format!("<raw DER, {} bytes>", der.len()))
}

/// Try to extract a human-readable issuer string from DER bytes.
/// Returns `None` if parsing fails at any point.
fn parse_issuer_from_tbs(der: &[u8]) -> Option<String> {
    let pos = navigate_to_issuer(der)?;
    let (_, issuer_bytes) = parse_der_sequence(pos)?;

    // Walk the RDN SEQUENCEs and extract OID=value pairs
    extract_rdns(issuer_bytes)
}

/// Parse a DER SEQUENCE tag+length, returning (rest after, content).
fn parse_der_sequence(data: &[u8]) -> Option<(&[u8], &[u8])> {
    if data.first()? != &0x30 {
        return None;
    }
    let (rest, content_len) = parse_der_length(&data[1..])?;
    if rest.len() < content_len {
        return None;
    }
    Some((&rest[content_len..], &rest[..content_len]))
}

/// Skip one DER element (tag + length + value), returning the
/// remainder of the slice.
fn skip_der_element(data: &[u8]) -> Option<(&[u8], &[u8])> {
    if data.is_empty() {
        return None;
    }
    let (rest, content_len) = parse_der_length(&data[1..])?;
    if rest.len() < content_len {
        return None;
    }
    Some((&rest[content_len..], &rest[..content_len]))
}

/// Parse a DER length encoding, returning (rest, length value).
fn parse_der_length(data: &[u8]) -> Option<(&[u8], usize)> {
    let first = *data.first()?;
    if first < 0x80 {
        Some((&data[1..], first as usize))
    } else {
        let num_bytes = (first & 0x7f) as usize;
        if num_bytes == 0 || num_bytes > 4 || data.len() < 1 + num_bytes {
            return None;
        }
        let mut len: usize = 0;
        for &b in &data[1..=num_bytes] {
            len = len.checked_shl(8)?.checked_add(b as usize)?;
        }
        Some((&data[1 + num_bytes..], len))
    }
}

/// Walk RDN SET/SEQUENCE structures and produce "CN=foo, O=bar" style
/// output. Falls back to hex if UTF-8 decoding fails.
fn extract_rdns(mut data: &[u8]) -> Option<String> {
    let mut parts = Vec::new();

    while !data.is_empty() {
        // Each RDN is a SET
        let set_tag = *data.first()?;
        if set_tag != 0x31 {
            break;
        }
        let (rest, set_content) = skip_der_element(data)?;
        data = rest;

        // Inside the SET is a SEQUENCE of OID + value
        if let Some((_, seq_content)) = parse_der_sequence(set_content) {
            if let Some(part) = parse_attribute_type_and_value(seq_content) {
                parts.push(part);
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

/// Parse an `AttributeTypeAndValue` (OID + string value).
fn parse_attribute_type_and_value(data: &[u8]) -> Option<String> {
    // OID tag = 0x06
    if data.first()? != &0x06 {
        return None;
    }
    let (rest, oid_bytes) = skip_der_element(data)?;
    let oid_name = oid_short_name(oid_bytes);

    // Value is a string type (UTF8String 0x0C, PrintableString 0x13,
    // IA5String 0x16, etc.)
    let (_, value_bytes) = skip_der_element(rest)?;
    let value =
        String::from_utf8(value_bytes.to_vec()).unwrap_or_else(|_| hex::encode(value_bytes));

    Some(format!("{oid_name}={value}"))
}

/// Map common X.500 OID byte sequences to short names.
fn oid_short_name(oid: &[u8]) -> &'static str {
    match oid {
        // 2.5.4.3 — CN
        [0x55, 0x04, 0x03] => "CN",
        // 2.5.4.6 — C
        [0x55, 0x04, 0x06] => "C",
        // 2.5.4.7 — L
        [0x55, 0x04, 0x07] => "L",
        // 2.5.4.8 — ST
        [0x55, 0x04, 0x08] => "ST",
        // 2.5.4.10 — O
        [0x55, 0x04, 0x0a] => "O",
        // 2.5.4.11 — OU
        [0x55, 0x04, 0x0b] => "OU",
        _ => "OID",
    }
}

/// Simple hex encoder to avoid adding a dependency just for error
/// messages in the DER parser fallback path.
mod hex {
    use std::fmt::Write as _;

    pub(super) fn encode(data: &[u8]) -> String {
        let mut s = String::with_capacity(data.len() * 2);
        for b in data {
            let _ = write!(s, "{b:02x}");
        }
        s
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::tls::fingerprint::compute_fingerprint;

    /// Generate a self-signed certificate using `rcgen` and return
    /// the DER-encoded certificate bytes.
    fn gen_self_signed_cert() -> Vec<u8> {
        let params = rcgen::CertificateParams::new(vec!["localhost".to_owned()]).unwrap();
        let cert = params
            .self_signed(&rcgen::KeyPair::generate().unwrap())
            .unwrap();
        cert.der().to_vec()
    }

    #[test]
    fn pinned_verifier_accepts_matching_cert() {
        let der = gen_self_signed_cert();
        let fp = compute_fingerprint(&der);

        let verifier = PinnedCertVerifier::new(&fp, None, None, "localhost").unwrap();

        let cert = CertificateDer::from(der);
        let server_name = ServerName::try_from("localhost").unwrap();

        let result = verifier.verify_server_cert(&cert, &[], &server_name, &[], UnixTime::now());

        assert!(
            result.is_ok(),
            "matching pin should be accepted: {result:?}"
        );
    }

    #[test]
    fn pinned_verifier_rejects_mismatched_cert() {
        // Create a verifier pinned to one cert, present a different one
        let der1 = gen_self_signed_cert();
        let fp1 = compute_fingerprint(&der1);

        let der2 = gen_self_signed_cert();

        let verifier = PinnedCertVerifier::new(&fp1, None, None, "localhost").unwrap();

        let cert = CertificateDer::from(der2);
        let server_name = ServerName::try_from("localhost").unwrap();

        let result = verifier.verify_server_cert(&cert, &[], &server_name, &[], UnixTime::now());

        assert!(result.is_err(), "mismatched pin should be rejected");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("PIN_MISMATCH"),
            "error should contain PIN_MISMATCH: {err_msg}"
        );
    }

    #[test]
    fn ca_cert_config_rejects_missing_file() {
        let result = build_ca_cert_config(Path::new("/nonexistent/ca.pem"));
        assert!(result.is_err(), "missing file should fail");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("failed to read"),
            "error should mention 'failed to read': {err_msg}"
        );
    }

    #[test]
    fn build_pinned_config_succeeds() {
        let der = gen_self_signed_cert();
        let fp = compute_fingerprint(&der);
        let result = build_pinned_config(&fp, None, None, "localhost");
        assert!(
            result.is_ok(),
            "build_pinned_config should succeed: {result:?}"
        );
    }

    #[test]
    fn extract_issuer_dn_returns_fallback_for_garbage() {
        let result = extract_issuer_dn(b"not a certificate");
        assert!(
            result.contains("raw DER"),
            "garbage input should produce fallback: {result}"
        );
    }

    #[test]
    fn extract_issuer_dn_parses_rcgen_cert() {
        let der = gen_self_signed_cert();
        let issuer = extract_issuer_dn(&der);
        // rcgen self-signed certs have CN=localhost as issuer
        assert!(
            issuer.contains("CN="),
            "should extract CN from issuer: {issuer}"
        );
    }

    #[test]
    fn pinned_verifier_accepts_matching_pin_regardless_of_issuer() {
        // Pin match always succeeds — even if the stored issuer differs.
        let der = gen_self_signed_cert();
        let fp = compute_fingerprint(&der);

        let verifier =
            PinnedCertVerifier::new(&fp, Some("CN=SomeOtherCA".to_owned()), None, "localhost")
                .unwrap();

        let cert = CertificateDer::from(der);
        let server_name = ServerName::try_from("localhost").unwrap();

        let result = verifier.verify_server_cert(&cert, &[], &server_name, &[], UnixTime::now());

        assert!(
            result.is_ok(),
            "matching pin should always be accepted: {result:?}"
        );
    }

    /// Generate a self-signed certificate with a custom CN.
    fn gen_cert_with_cn(cn: &str) -> Vec<u8> {
        let mut params = rcgen::CertificateParams::new(vec![cn.to_owned()]).unwrap();
        let mut dn = rcgen::DistinguishedName::new();
        dn.push(rcgen::DnType::CommonName, cn);
        params.distinguished_name = dn;
        let cert = params
            .self_signed(&rcgen::KeyPair::generate().unwrap())
            .unwrap();
        cert.der().to_vec()
    }

    #[test]
    fn pinned_verifier_detects_issuer_change() {
        // Pin mismatch + different issuer → ISSUER_CHANGED
        let der1 = gen_cert_with_cn("OriginalCA");
        let fp1 = compute_fingerprint(&der1);

        // Pin to cert1 with cert1's issuer
        let issuer1 = extract_issuer_dn(&der1);
        let verifier = PinnedCertVerifier::new(&fp1, Some(issuer1), None, "localhost").unwrap();

        // Present a cert with a different CN (different issuer)
        let der2 = gen_cert_with_cn("EvilCA");
        let cert2 = CertificateDer::from(der2);
        let server_name = ServerName::try_from("localhost").unwrap();

        let result = verifier.verify_server_cert(&cert2, &[], &server_name, &[], UnixTime::now());

        assert!(result.is_err(), "issuer change should be rejected");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("ISSUER_CHANGED"),
            "error should contain ISSUER_CHANGED: {err_msg}"
        );
    }

    #[test]
    fn extract_issuer_der_returns_consistent_bytes() {
        let der = gen_self_signed_cert();
        let issuer1 = extract_issuer_der(&der);
        let issuer2 = extract_issuer_der(&der);
        assert_eq!(issuer1, issuer2, "should be deterministic");
        assert!(issuer1.is_some(), "should extract from valid cert");
    }

    #[test]
    fn extract_issuer_der_differs_for_different_issuers() {
        let der1 = gen_cert_with_cn("CA One");
        let der2 = gen_cert_with_cn("CA Two");
        let issuer1 = extract_issuer_der(&der1).unwrap();
        let issuer2 = extract_issuer_der(&der2).unwrap();
        assert_ne!(issuer1, issuer2, "different CAs should have different DER");
    }

    #[test]
    fn extract_issuer_der_returns_none_for_garbage() {
        assert!(extract_issuer_der(b"not a certificate").is_none());
    }

    #[test]
    fn pinned_verifier_detects_issuer_change_via_der() {
        // Pin mismatch + different issuer DER → ISSUER_CHANGED
        let der1 = gen_cert_with_cn("OriginalCA");
        let fp1 = compute_fingerprint(&der1);
        let issuer_der_bytes = extract_issuer_der(&der1).unwrap();
        let issuer_der_b64 = base64::engine::general_purpose::STANDARD.encode(&issuer_der_bytes);

        let verifier =
            PinnedCertVerifier::new(&fp1, None, Some(&issuer_der_b64), "localhost").unwrap();

        // Present a cert with a different CN (different issuer DER)
        let der2 = gen_cert_with_cn("EvilCA");
        let cert2 = CertificateDer::from(der2);
        let server_name = ServerName::try_from("localhost").unwrap();

        let result = verifier.verify_server_cert(&cert2, &[], &server_name, &[], UnixTime::now());

        assert!(result.is_err(), "issuer DER change should be rejected");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("ISSUER_CHANGED"),
            "error should contain ISSUER_CHANGED: {err_msg}"
        );
    }

    #[test]
    fn pinned_verifier_allows_pin_mismatch_with_same_issuer_der() {
        // Pin mismatch but same issuer DER → PIN_MISMATCH (not ISSUER_CHANGED)
        let der1 = gen_self_signed_cert();
        let fp1 = compute_fingerprint(&der1);
        let issuer_der_bytes = extract_issuer_der(&der1).unwrap();
        let issuer_der_b64 = base64::engine::general_purpose::STANDARD.encode(&issuer_der_bytes);

        let verifier =
            PinnedCertVerifier::new(&fp1, None, Some(&issuer_der_b64), "localhost").unwrap();

        // Both certs are self-signed with CN=localhost (same issuer)
        let der2 = gen_self_signed_cert();
        let cert2 = CertificateDer::from(der2);
        let server_name = ServerName::try_from("localhost").unwrap();

        let result = verifier.verify_server_cert(&cert2, &[], &server_name, &[], UnixTime::now());

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("PIN_MISMATCH"),
            "same issuer DER should produce PIN_MISMATCH: {err_msg}"
        );
    }

    #[test]
    fn pinned_verifier_rejects_invalid_base64_issuer_der() {
        // Invalid base64 → config error
        let der = gen_self_signed_cert();
        let fp = compute_fingerprint(&der);
        let result =
            PinnedCertVerifier::new(&fp, None, Some("!!!not-valid-base64!!!"), "localhost");
        assert!(result.is_err(), "invalid base64 should be rejected");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("invalid base64 in tls_pin_issuer_der"),
            "error should mention invalid base64: {err_msg}"
        );
    }

    /// Generate a self-signed certificate and return it as a PEM string.
    fn gen_self_signed_pem() -> String {
        let params = rcgen::CertificateParams::new(vec!["localhost".to_owned()]).unwrap();
        let cert = params
            .self_signed(&rcgen::KeyPair::generate().unwrap())
            .unwrap();
        cert.pem()
    }

    #[test]
    fn ca_cert_config_loads_valid_pem() {
        // Happy path: a real PEM-encoded self-signed cert is accepted as a CA
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let pem = gen_self_signed_pem();
        std::fs::write(tmp.path(), pem).unwrap();
        let result = build_ca_cert_config(tmp.path());
        assert!(
            result.is_ok(),
            "valid PEM should produce a config: {result:?}"
        );
    }

    #[test]
    fn ca_cert_config_rejects_empty_pem_file() {
        // Empty file → no certs found
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "").unwrap();
        let result = build_ca_cert_config(tmp.path());
        assert!(result.is_err(), "empty PEM file should be rejected");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("no valid PEM certificates found"),
            "error should mention missing certs: {err_msg}"
        );
    }

    #[test]
    fn ca_cert_config_rejects_malformed_pem() {
        // PEM markers but garbage body → parse failure
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            tmp.path(),
            "-----BEGIN CERTIFICATE-----\nnot valid base64 here !!!\n-----END CERTIFICATE-----\n",
        )
        .unwrap();
        let result = build_ca_cert_config(tmp.path());
        assert!(result.is_err(), "malformed PEM should be rejected");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("failed to parse PEM certificates")
                || err_msg.contains("no valid PEM certificates found"),
            "error should mention parse failure or missing certs: {err_msg}"
        );
    }

    #[test]
    fn parse_der_length_short_form() {
        // Short form: first byte < 0x80 is the length itself
        let data = [0x05_u8, 0x01, 0x02, 0x03];
        let (rest, len) = parse_der_length(&data).unwrap();
        assert_eq!(len, 5);
        assert_eq!(rest, &[0x01, 0x02, 0x03]);
    }

    #[test]
    fn parse_der_length_long_form_two_bytes() {
        // Long form: 0x82 = 2 length bytes follow; 0x01 0x00 = 256
        let data = [0x82_u8, 0x01, 0x00, 0xaa];
        let (rest, len) = parse_der_length(&data).unwrap();
        assert_eq!(len, 256);
        assert_eq!(rest, &[0xaa]);
    }

    #[test]
    fn parse_der_length_rejects_indefinite_form() {
        // 0x80 indicates indefinite length (num_bytes == 0): not allowed
        let data = [0x80_u8];
        assert!(parse_der_length(&data).is_none());
    }

    #[test]
    fn parse_der_length_rejects_too_many_length_bytes() {
        // 0x85 = 5 length bytes follow, but we cap at 4
        let data = [0x85_u8, 0x00, 0x00, 0x00, 0x00, 0x01];
        assert!(parse_der_length(&data).is_none());
    }

    #[test]
    fn parse_der_length_rejects_truncated_long_form() {
        // 0x82 = 2 length bytes follow, but only 1 is present
        let data = [0x82_u8, 0x01];
        assert!(parse_der_length(&data).is_none());
    }

    #[test]
    fn parse_der_sequence_rejects_wrong_tag() {
        // Anything that doesn't start with 0x30 is not a SEQUENCE
        let data = [0x02_u8, 0x01, 0x05]; // INTEGER 5
        assert!(parse_der_sequence(&data).is_none());
    }

    #[test]
    fn parse_der_sequence_rejects_truncated_content() {
        // SEQUENCE tag with length 5 but only 2 content bytes
        let data = [0x30_u8, 0x05, 0x01, 0x02];
        assert!(parse_der_sequence(&data).is_none());
    }

    #[test]
    fn skip_der_element_rejects_empty() {
        assert!(skip_der_element(&[]).is_none());
    }

    #[test]
    fn skip_der_element_rejects_truncated() {
        // Tag + claimed length 10 but only 2 content bytes
        let data = [0x04_u8, 0x0a, 0x01, 0x02];
        assert!(skip_der_element(&data).is_none());
    }

    #[test]
    fn oid_short_name_maps_known_oids() {
        // 2.5.4.3 — CN
        assert_eq!(oid_short_name(&[0x55, 0x04, 0x03]), "CN");
        // 2.5.4.6 — C
        assert_eq!(oid_short_name(&[0x55, 0x04, 0x06]), "C");
        // 2.5.4.7 — L
        assert_eq!(oid_short_name(&[0x55, 0x04, 0x07]), "L");
        // 2.5.4.8 — ST
        assert_eq!(oid_short_name(&[0x55, 0x04, 0x08]), "ST");
        // 2.5.4.10 — O
        assert_eq!(oid_short_name(&[0x55, 0x04, 0x0a]), "O");
        // 2.5.4.11 — OU
        assert_eq!(oid_short_name(&[0x55, 0x04, 0x0b]), "OU");
        // unknown → fallback
        assert_eq!(oid_short_name(&[0x55, 0x04, 0xff]), "OID");
        assert_eq!(oid_short_name(&[]), "OID");
    }

    #[test]
    fn parse_attribute_type_and_value_rejects_non_oid() {
        // Starts with INTEGER (0x02), not OID (0x06)
        let data = [0x02_u8, 0x01, 0x05];
        assert!(parse_attribute_type_and_value(&data).is_none());
    }

    #[test]
    fn parse_attribute_type_and_value_falls_back_to_hex_for_non_utf8() {
        // OID 2.5.4.3 (CN) followed by an OCTET STRING with invalid UTF-8
        let mut data = Vec::new();
        // OID tag, length 3, body 2.5.4.3
        data.extend_from_slice(&[0x06, 0x03, 0x55, 0x04, 0x03]);
        // OCTET STRING tag, length 2, invalid UTF-8 bytes
        data.extend_from_slice(&[0x04, 0x02, 0xff, 0xfe]);
        let result = parse_attribute_type_and_value(&data).unwrap();
        // Should be "CN=fffe" (hex-encoded fallback)
        assert_eq!(result, "CN=fffe");
    }

    #[test]
    fn extract_rdns_breaks_on_non_set_tag() {
        // Starts with SEQUENCE (0x30) instead of SET (0x31)
        let data = [0x30_u8, 0x00];
        // Should hit the `break` branch and then return None (no parts)
        assert!(extract_rdns(&data).is_none());
    }

    #[test]
    fn extract_rdns_returns_none_on_empty_input() {
        assert!(extract_rdns(&[]).is_none());
    }

    #[test]
    fn hex_encode_produces_lowercase_pairs() {
        assert_eq!(hex::encode(&[0x00, 0x0f, 0xff, 0xab]), "000fffab");
        assert_eq!(hex::encode(&[]), "");
    }
}
