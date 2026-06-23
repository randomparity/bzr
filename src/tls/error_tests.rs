#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn looks_like_tls_error_matches_cert_keyword() {
    assert!(looks_like_tls_error("certificate verify failed"));
}

#[test]
fn looks_like_tls_error_matches_ssl_keyword() {
    assert!(looks_like_tls_error("SSL handshake failure"));
}

#[test]
fn looks_like_tls_error_matches_tls_keyword() {
    assert!(looks_like_tls_error("TLS protocol error"));
}

#[test]
fn looks_like_tls_error_rejects_unrelated_message() {
    assert!(!looks_like_tls_error("connection refused"));
}

#[test]
fn is_connect_tls_error_true_when_connect_and_tls_keyword() {
    assert!(is_connect_tls_error(true, "tls handshake failed"));
}

#[test]
fn is_connect_tls_error_false_when_not_connect() {
    assert!(!is_connect_tls_error(false, "tls handshake failed"));
}

#[test]
fn is_connect_tls_error_false_without_tls_keyword() {
    assert!(!is_connect_tls_error(true, "connection refused"));
}

#[tokio::test]
async fn tls_hint_no_hint_for_non_tls_error() {
    let client = crate::tls::build_tls_client(&crate::tls::TlsConfig::default()).unwrap();
    let err = client
        .get("http://127.0.0.1:1/nope")
        .send()
        .await
        .unwrap_err();
    let result = tls_hint("connection failed", &err);
    assert_eq!(result, "connection failed");
}
