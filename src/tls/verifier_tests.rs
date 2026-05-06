#![expect(clippy::unwrap_used)]

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
fn pinned_verifier_advertises_default_signature_schemes() {
    // The verifier delegates `supported_verify_schemes` to the inner
    // signature verifier, which must return rustls's default scheme set
    // (an empty vec would silently break TLS handshakes by claiming we
    // support no signatures).
    use rustls::client::danger::ServerCertVerifier;
    let der = gen_self_signed_cert();
    let fp = compute_fingerprint(&der);
    let verifier = PinnedCertVerifier::new(&fp, None, None, "localhost").unwrap();
    let schemes = verifier.supported_verify_schemes();
    assert!(
        !schemes.is_empty(),
        "verifier should advertise at least one signature scheme"
    );
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
        PinnedCertVerifier::new(&fp, Some("CN=SomeOtherCA".to_owned()), None, "localhost").unwrap();

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

    let verifier = PinnedCertVerifier::new(&fp1, None, Some(&issuer_der_b64), "localhost").unwrap();

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

    let verifier = PinnedCertVerifier::new(&fp1, None, Some(&issuer_der_b64), "localhost").unwrap();

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
    let result = PinnedCertVerifier::new(&fp, None, Some("!!!not-valid-base64!!!"), "localhost");
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
fn parse_der_length_long_form_one_byte() {
    // Long form: 0x81 = 1 length byte follows; 0x80 = 128 (>127, so
    // long form is required even for a single byte).
    let data = [0x81_u8, 0x80, 0xaa];
    let (rest, len) = parse_der_length(&data).unwrap();
    assert_eq!(len, 128);
    assert_eq!(rest, &[0xaa]);
}

#[test]
fn parse_der_length_rejects_indefinite_form() {
    // 0x80 indicates indefinite length (num_bytes == 0): not allowed
    let data = [0x80_u8];
    assert!(parse_der_length(&data).is_none());
}

#[test]
fn parse_der_length_long_form_four_bytes_with_exact_fit() {
    // 4 length bytes is the upper bound; data length exactly equals
    // `1 + num_bytes` (no content bytes after the length encoding).
    let data = [0x84_u8, 0x00, 0x00, 0x01, 0x00];
    let (rest, len) = parse_der_length(&data).unwrap();
    assert_eq!(len, 256);
    assert!(rest.is_empty());
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
