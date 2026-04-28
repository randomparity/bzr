use std::io::{self, IsTerminal, Write};
use std::sync::{Arc, Mutex};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};

use crate::error::{BzrError, Result};
use base64::Engine;

use crate::tls::fingerprint::compute_fingerprint;
use crate::tls::verifier::{extract_issuer_der, extract_issuer_dn};

/// A TLS verifier that accepts any certificate but captures the leaf
/// certificate DER bytes and issuer for TOFU inspection.
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
        let issuer = extract_issuer_dn(&der);
        #[expect(clippy::unwrap_used)]
        let mut guard = self.captured.lock().unwrap();
        *guard = Some((der, issuer));
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

/// Connect to a server with TLS verification disabled and capture the
/// leaf certificate. Returns `(fingerprint, issuer_dn, issuer_der_b64)`.
///
/// The third element is the base64-encoded raw DER bytes of the issuer
/// SEQUENCE, or `None` if DER extraction fails.
///
/// No authentication headers are sent — only a HEAD request is made.
pub(crate) async fn probe_server_cert(url: &str) -> Result<(String, String, Option<String>)> {
    let provider = super::default_provider();

    let capture = Arc::new(CertCapture {
        captured: Mutex::new(None),
        provider: provider.clone(),
    });

    let tls_config = super::base_tls_builder("for probing")?
        .dangerous()
        .with_custom_certificate_verifier(capture.clone())
        .with_no_client_auth();

    let client = reqwest::Client::builder()
        .use_preconfigured_tls(tls_config)
        .connect_timeout(crate::http::CONNECT_TIMEOUT)
        .timeout(crate::http::REQUEST_TIMEOUT)
        .build()
        .map_err(|e| BzrError::config(format!("failed to build TLS probe client: {e}")))?;

    client.head(url).send().await.map_err(|e| {
        BzrError::config(format!("failed to probe server certificate at {url}: {e}"))
    })?;

    #[expect(clippy::unwrap_used)]
    let guard = capture.captured.lock().unwrap();
    let (der, issuer) = guard
        .as_ref()
        .ok_or_else(|| BzrError::config(format!("no certificate captured from {url}")))?;

    let fingerprint = compute_fingerprint(der);
    let issuer_der_b64 = extract_issuer_der(der)
        .map(|bytes| base64::engine::general_purpose::STANDARD.encode(&bytes));
    Ok((fingerprint, issuer.clone(), issuer_der_b64))
}

/// Read a line from stdin if running interactively.
/// Returns `Ok(None)` when stdin is not a terminal.
fn read_interactive_line(prompt: &str) -> Result<Option<String>> {
    if !io::stdin().is_terminal() {
        return Ok(None);
    }
    let _ = write!(io::stderr(), "{prompt}");
    let _ = io::stderr().flush();
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| BzrError::config(format!("failed to read input: {e}")))?;
    Ok(Some(input.trim().to_string()))
}

/// Parse a TOFU response from user input.
///
/// Returns:
/// - `Some(true)` for "always" (persist the pin)
/// - `Some(false)` for "y"/"yes" (trust once)
/// - `None` for anything else (reject)
pub(crate) fn parse_tofu_response(input: &str) -> Option<bool> {
    match input.trim().to_ascii_lowercase().as_str() {
        "always" => Some(true),
        "y" | "yes" => Some(false),
        _ => None,
    }
}

/// Parse a yes/no response from user input.
/// Returns `true` for "y" or "yes" (case-insensitive).
pub(crate) fn parse_yes_no(input: &str) -> bool {
    input.trim().eq_ignore_ascii_case("y") || input.trim().eq_ignore_ascii_case("yes")
}

/// Prompt the user to confirm pinning a certificate. Returns `false`
/// if stdin is not a terminal.
pub(crate) fn confirm_pin() -> Result<bool> {
    let input = read_interactive_line("Pin this certificate? [y/N] ")?;
    Ok(input.as_deref().is_some_and(parse_yes_no))
}

/// Prompt the user for first-contact TOFU decision.
///
/// Returns:
/// - `Some(true)` for "always" (persist the pin)
/// - `Some(false)` for "y"/"yes" (trust once)
/// - `None` for anything else (reject) or non-interactive
pub(crate) fn prompt_tofu(
    server_name: &str,
    hostname: &str,
    fingerprint: &str,
    issuer: &str,
) -> Result<Option<bool>> {
    let _ = writeln!(io::stderr());
    let _ = writeln!(
        io::stderr(),
        "WARNING: No certificate pin on file for server \"{server_name}\" ({hostname})."
    );
    let _ = writeln!(io::stderr(), "  Fingerprint: {fingerprint}");
    let _ = writeln!(io::stderr(), "  Issuer:      {issuer}");
    let _ = writeln!(io::stderr());

    let Some(trimmed) = read_interactive_line("Trust this certificate? [y/N/always] ")? else {
        return Ok(None);
    };

    Ok(parse_tofu_response(&trimmed))
}

/// Prompt the user to accept a certificate rotation (pin changed).
/// Returns `false` if stdin is not a terminal or the user declines.
pub(crate) fn prompt_rotation(
    server_name: &str,
    hostname: &str,
    old_pin: &str,
    new_pin: &str,
    issuer: &str,
) -> Result<bool> {
    let _ = writeln!(io::stderr());
    let _ = writeln!(
        io::stderr(),
        "WARNING: Certificate changed for server \"{server_name}\" ({hostname})!"
    );
    let _ = writeln!(io::stderr(), "  Old pin: {old_pin}");
    let _ = writeln!(io::stderr(), "  New pin: {new_pin}");
    let _ = writeln!(io::stderr(), "  Issuer:  {issuer} (unchanged)");
    let _ = writeln!(io::stderr());

    let input = read_interactive_line("Accept the new certificate? [y/N] ")?;
    Ok(input.as_deref().is_some_and(parse_yes_no))
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

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
    fn parse_yes_no_accepts_y() {
        assert!(parse_yes_no("y"));
        assert!(parse_yes_no("Y"));
        assert!(parse_yes_no("yes"));
        assert!(parse_yes_no("YES"));
        assert!(parse_yes_no("  y  "));
    }

    #[test]
    fn parse_yes_no_rejects_others() {
        assert!(!parse_yes_no("n"));
        assert!(!parse_yes_no(""));
        assert!(!parse_yes_no("no"));
        assert!(!parse_yes_no("anything"));
    }
}
