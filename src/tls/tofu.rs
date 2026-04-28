use std::io::{self, IsTerminal, Write};
use std::sync::{Arc, Mutex};

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};

use crate::error::{BzrError, Result};
use crate::tls::fingerprint::compute_fingerprint;
use crate::tls::verifier::extract_issuer_dn;

/// A TLS verifier that accepts any certificate but captures the leaf
/// certificate DER bytes and issuer for TOFU inspection.
#[derive(Debug)]
struct CertCapture {
    captured: Mutex<Option<(Vec<u8>, String)>>,
    provider: Arc<CryptoProvider>,
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
/// leaf certificate. Returns `(fingerprint, issuer_dn)`.
///
/// No authentication headers are sent — only a HEAD request is made.
pub(crate) async fn probe_server_cert(url: &str) -> Result<(String, String)> {
    let provider = CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::ring::default_provider()));

    let capture = Arc::new(CertCapture {
        captured: Mutex::new(None),
        provider: provider.clone(),
    });

    let tls_config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| BzrError::config(format!("failed to configure TLS for probing: {e}")))?
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
    Ok((fingerprint, issuer.clone()))
}

/// Prompt the user to confirm pinning a certificate. Returns `false`
/// if stdin is not a terminal.
pub(crate) fn confirm_pin() -> Result<bool> {
    if !io::stdin().is_terminal() {
        return Ok(false);
    }

    let _ = write!(io::stderr(), "Pin this certificate? [y/N] ");
    let _ = io::stderr().flush();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| BzrError::config(format!("failed to read input: {e}")))?;

    Ok(input.trim().eq_ignore_ascii_case("y"))
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
    if !io::stdin().is_terminal() {
        return Ok(None);
    }

    let _ = writeln!(io::stderr());
    let _ = writeln!(
        io::stderr(),
        "WARNING: No certificate pin on file for server \"{server_name}\" ({hostname})."
    );
    let _ = writeln!(io::stderr(), "  Fingerprint: {fingerprint}");
    let _ = writeln!(io::stderr(), "  Issuer:      {issuer}");
    let _ = writeln!(io::stderr());
    let _ = write!(io::stderr(), "Trust this certificate? [y/N/always] ");
    let _ = io::stderr().flush();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| BzrError::config(format!("failed to read input: {e}")))?;

    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("always") {
        Ok(Some(true))
    } else if trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes") {
        Ok(Some(false))
    } else {
        Ok(None)
    }
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
    if !io::stdin().is_terminal() {
        return Ok(false);
    }

    let _ = writeln!(io::stderr());
    let _ = writeln!(
        io::stderr(),
        "WARNING: Certificate changed for server \"{server_name}\" ({hostname})!"
    );
    let _ = writeln!(io::stderr(), "  Old pin: {old_pin}");
    let _ = writeln!(io::stderr(), "  New pin: {new_pin}");
    let _ = writeln!(io::stderr(), "  Issuer:  {issuer} (unchanged)");
    let _ = writeln!(io::stderr());
    let _ = write!(io::stderr(), "Accept the new certificate? [y/N] ");
    let _ = io::stderr().flush();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| BzrError::config(format!("failed to read input: {e}")))?;

    Ok(input.trim().eq_ignore_ascii_case("y"))
}
