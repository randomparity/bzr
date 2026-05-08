use std::path::PathBuf;
use std::sync::Arc;

pub(crate) mod fingerprint;
pub(crate) mod pin_failure;
pub(crate) mod tofu;
pub(crate) mod verifier;

/// Get the default crypto provider, falling back to ring.
pub(crate) fn default_provider() -> Arc<rustls::crypto::CryptoProvider> {
    rustls::crypto::CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::ring::default_provider()))
}

/// Create a `rustls::ConfigBuilder` with the default provider and safe
/// protocol versions. Shared by `build_ca_cert_config`,
/// `build_pinned_config`, and `probe_server_cert`.
pub(crate) fn base_tls_builder(
    context: &str,
) -> crate::error::Result<rustls::ConfigBuilder<rustls::ClientConfig, rustls::WantsVerifier>> {
    rustls::ClientConfig::builder_with_provider(default_provider())
        .with_safe_default_protocol_versions()
        .map_err(|e| {
            crate::error::BzrError::config(format!("failed to configure TLS {context}: {e}"))
        })
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

/// Apply the verification mode encoded in `TlsConfig` to a builder.
///
/// Selects:
/// 1. `insecure` — accept all certs (`danger_accept_invalid_certs`)
/// 2. `ca_cert_path` — custom CA added to root store
/// 3. `pin_sha256` — pinned certificate fingerprint verification
/// 4. None — default system roots
fn apply_tls_verification(
    mut builder: reqwest::ClientBuilder,
    config: &TlsConfig,
) -> crate::error::Result<reqwest::ClientBuilder> {
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
    Ok(builder)
}

/// Build a `reqwest::Client` with the configured TLS verification mode.
/// Used for real API calls; follows redirects per reqwest defaults.
pub fn build_tls_client(config: &TlsConfig) -> crate::error::Result<reqwest::Client> {
    let builder = reqwest::Client::builder()
        .connect_timeout(crate::http::CONNECT_TIMEOUT)
        .timeout(crate::http::REQUEST_TIMEOUT);
    apply_tls_verification(builder, config)?
        .build()
        .map_err(crate::error::BzrError::Http)
}

/// Build a `reqwest::Client` for cert-detection probes.
///
/// Same TLS verification mode as `build_tls_client`, but redirects are
/// disabled so the probe only validates the certificate presented by the
/// configured URL itself. Following a redirect off-host would surface a
/// TLS error (or pin mismatch) for an endpoint the user did not
/// configure, and any subsequent prompt would describe one host while
/// trusting another.
pub(crate) fn build_probe_client(config: &TlsConfig) -> crate::error::Result<reqwest::Client> {
    let builder = reqwest::Client::builder()
        .connect_timeout(crate::http::CONNECT_TIMEOUT)
        .timeout(crate::http::REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none());
    apply_tls_verification(builder, config)?
        .build()
        .map_err(crate::error::BzrError::Http)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
