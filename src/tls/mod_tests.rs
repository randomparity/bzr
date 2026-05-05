#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn build_tls_client_default_succeeds() {
    let client = build_tls_client(&TlsConfig::default());
    assert!(client.is_ok());
}

#[test]
fn build_tls_client_insecure_succeeds() {
    let config = TlsConfig {
        insecure: true,
        ..Default::default()
    };
    assert!(build_tls_client(&config).is_ok());
}

#[test]
fn build_tls_client_pinned_succeeds() {
    let config = TlsConfig {
        pin_sha256: Some(crate::tls::fingerprint::compute_fingerprint(b"test")),
        server_name: Some("test".into()),
        ..Default::default()
    };
    assert!(build_tls_client(&config).is_ok());
}

#[test]
fn build_tls_client_bad_pin_fails() {
    let config = TlsConfig {
        pin_sha256: Some("not-a-valid-pin".into()),
        ..Default::default()
    };
    assert!(build_tls_client(&config).is_err());
}

#[test]
fn build_tls_client_missing_ca_cert_fails() {
    let config = TlsConfig {
        ca_cert_path: Some("/nonexistent/ca.pem".into()),
        ..Default::default()
    };
    let err = build_tls_client(&config).unwrap_err();
    assert!(
        err.to_string().contains("failed to read"),
        "should report missing file: {err}"
    );
}
