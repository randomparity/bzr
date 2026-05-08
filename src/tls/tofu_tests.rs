#![expect(clippy::unwrap_used)]

use super::*;
use rustls::client::danger::ServerCertVerifier;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};

#[test]
fn cert_capture_accepts_any_cert() {
    let provider = crate::tls::default_provider();
    let capture = CertCapture {
        captured: OnceLock::new(),
        provider,
    };
    let cert_data = b"fake cert data";
    let cert = CertificateDer::from(cert_data.to_vec());
    let server_name = ServerName::try_from("localhost").unwrap();

    let result = capture.verify_server_cert(&cert, &[], &server_name, &[], UnixTime::now());
    assert!(result.is_ok(), "CertCapture should accept any cert");

    let (der, _issuer) = capture.captured.get().unwrap();
    assert_eq!(der, cert_data, "captured DER should match input");
}

#[test]
fn cert_capture_supported_verify_schemes_not_empty() {
    let provider = crate::tls::default_provider();
    let capture = CertCapture {
        captured: OnceLock::new(),
        provider,
    };
    assert!(
        !capture.supported_verify_schemes().is_empty(),
        "should expose provider's supported schemes"
    );
}

#[test]
fn read_interactive_line_returns_none_in_tests() {
    let result = read_interactive_line("prompt> ").unwrap();
    assert!(
        result.is_none(),
        "should return None when stdin is not a terminal"
    );
}

#[test]
fn confirm_pin_returns_false_non_interactive() {
    let result = confirm_pin().unwrap();
    assert!(!result);
}

#[test]
fn prompt_tofu_returns_none_non_interactive() {
    let result = prompt_tofu("test", "example.com", "sha256//abc", "CN=Test").unwrap();
    assert!(result.is_none());
}

#[test]
fn prompt_rotation_returns_false_non_interactive() {
    let result = prompt_rotation(
        "test",
        "example.com",
        "sha256//old",
        "sha256//new",
        "CN=Test",
    )
    .unwrap();
    assert!(!result);
}

#[tokio::test]
async fn probe_server_cert_returns_error_for_unreachable() {
    let result = probe_server_cert("https://127.0.0.1:1/unreachable").await;
    assert!(result.is_err(), "should fail for unreachable server");
}

#[test]
fn parse_tofu_response_always() {
    assert_eq!(parse_tofu_response("always"), Some(true));
    assert_eq!(parse_tofu_response("ALWAYS"), Some(true));
    assert_eq!(parse_tofu_response("  always  "), Some(true));
}

#[test]
fn parse_tofu_response_yes() {
    assert_eq!(parse_tofu_response("y"), Some(false));
    assert_eq!(parse_tofu_response("Y"), Some(false));
    assert_eq!(parse_tofu_response("yes"), Some(false));
    assert_eq!(parse_tofu_response("YES"), Some(false));
}

#[test]
fn parse_tofu_response_rejects_other() {
    assert_eq!(parse_tofu_response("n"), None);
    assert_eq!(parse_tofu_response(""), None);
    assert_eq!(parse_tofu_response("no"), None);
    assert_eq!(parse_tofu_response("anything"), None);
}

#[test]
fn is_yes_response_accepts_y() {
    assert!(is_yes_response("y"));
    assert!(is_yes_response("Y"));
    assert!(is_yes_response("yes"));
    assert!(is_yes_response("YES"));
    assert!(is_yes_response("  y  "));
}

#[test]
fn is_yes_response_rejects_others() {
    assert!(!is_yes_response("n"));
    assert!(!is_yes_response(""));
    assert!(!is_yes_response("no"));
    assert!(!is_yes_response("anything"));
}

/// Construct a `DigitallySignedStruct` for tests covering `CertCapture`'s
/// `verify_tls12_signature` and `verify_tls13_signature` trait methods.
///
/// `DigitallySignedStruct::new` is `pub(crate)` in rustls, so there is no
/// stable public constructor. The only way to build one in an external crate
/// without a live TLS handshake is via the wire-format codec — exposed only
/// through `rustls::internal::msgs::codec::Codec`, which rustls explicitly
/// marks `#[doc(hidden)]` and "DO NOT form part of the stable interface."
///
/// **Risk-vs-reward:** the two methods we cover are one-line delegates that
/// return `Ok(HandshakeSignatureValid::assertion())` — they cannot be wrong.
/// Without these tests, `tofu.rs` line coverage drops to ~82.5%, below the
/// project's 85% per-file floor; the remaining gaps (interactive stdin in
/// `prompt_tofu`/`prompt_rotation`, the post-handshake success path in
/// `probe_server_cert`) require infrastructure this project doesn't have.
///
/// If a future rustls upgrade reorganizes `internal::msgs::codec`, CI will
/// break loudly. The right response is to delete this helper and the two
/// `cert_capture_verify_*` tests, and accept the lower `tofu.rs` coverage
/// with a documented exception in the `SonarCloud` gate config.
fn dummy_dss() -> DigitallySignedStruct {
    use rustls::internal::msgs::codec::Codec;
    // ED25519 (0x0807) with empty signature.
    let bytes = [0x08_u8, 0x07, 0x00, 0x00];
    DigitallySignedStruct::read_bytes(&bytes).unwrap()
}

#[test]
fn cert_capture_verify_tls12_signature_returns_ok() {
    let provider = crate::tls::default_provider();
    let capture = CertCapture {
        captured: OnceLock::new(),
        provider,
    };
    let cert = CertificateDer::from(b"fake".to_vec());
    let dss = dummy_dss();
    let result = capture.verify_tls12_signature(b"msg", &cert, &dss);
    assert!(result.is_ok(), "tls12 signature should be accepted");
}

#[test]
fn cert_capture_verify_tls13_signature_returns_ok() {
    let provider = crate::tls::default_provider();
    let capture = CertCapture {
        captured: OnceLock::new(),
        provider,
    };
    let cert = CertificateDer::from(b"fake".to_vec());
    let dss = dummy_dss();
    let result = capture.verify_tls13_signature(b"msg", &cert, &dss);
    assert!(result.is_ok(), "tls13 signature should be accepted");
}
