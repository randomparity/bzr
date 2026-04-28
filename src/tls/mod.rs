use std::path::PathBuf;
use std::sync::Arc;

pub(crate) mod fingerprint;
pub(crate) mod tofu;
pub(crate) mod verifier;

/// Get the default crypto provider, falling back to ring.
pub(crate) fn default_provider() -> Arc<rustls::crypto::CryptoProvider> {
    rustls::crypto::CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::ring::default_provider()))
}

/// TLS configuration for a server connection.
#[derive(Debug, Clone, Default)]
pub struct TlsConfig {
    pub insecure: bool,
    pub ca_cert_path: Option<PathBuf>,
    pub pin_sha256: Option<String>,
    pub pin_issuer: Option<String>,
    /// Base64-encoded raw DER bytes of the pinned issuer SEQUENCE.
    pub pin_issuer_der: Option<String>,
    pub server_name: Option<String>,
}

/// Build a `reqwest::Client` with the appropriate TLS configuration.
///
/// Selects the verification mode based on `TlsConfig` fields:
/// 1. `insecure` — accept all certs (`danger_accept_invalid_certs`)
/// 2. `ca_cert_path` — custom CA added to root store
/// 3. `pin_sha256` — pinned certificate fingerprint verification
/// 4. None — default system roots
pub fn build_tls_client(config: &TlsConfig) -> crate::error::Result<reqwest::Client> {
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
            config.pin_issuer_der.as_deref(),
            config.server_name.as_deref().unwrap_or("unknown"),
        )?;
        builder = builder.use_preconfigured_tls(tls_config);
    }

    builder.build().map_err(crate::error::BzrError::Http)
}
