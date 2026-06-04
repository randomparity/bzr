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
#[non_exhaustive]
pub struct TlsConfig {
    pub insecure: bool,
    pub ca_cert_path: Option<PathBuf>,
    pub pin_sha256: Option<String>,
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
            config.pin_issuer_der.as_deref(),
            config.server_name.as_deref().unwrap_or("unknown"),
        )?;
        builder = builder.use_preconfigured_tls(tls_config);
    }
    Ok(builder)
}

/// Maximum redirect hops followed by the API client, matching reqwest's
/// default limit.
const MAX_REDIRECTS: usize = 10;

/// A redirect policy that follows redirects only while the target host
/// matches the host of the original request, refusing any cross-host hop.
///
/// reqwest strips its own known-sensitive headers (`Authorization`, `Cookie`,
/// …) on a cross-host redirect but not custom ones, so without this the
/// `X-BUGZILLA-API-KEY` header would be forwarded to a redirect-target host
/// and leak the API key. Same-host redirects are still followed, up to
/// [`MAX_REDIRECTS`].
fn same_host_redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        let origin_host = attempt.previous().first().and_then(|u| u.host_str());
        let cross_host = origin_host != attempt.url().host_str();
        let target = attempt.url().clone();
        if cross_host {
            return attempt.error(format!(
                "refusing to follow cross-host redirect to {target} (would leak credentials)"
            ));
        }
        if attempt.previous().len() >= MAX_REDIRECTS {
            return attempt.error(format!("exceeded {MAX_REDIRECTS} redirects"));
        }
        attempt.follow()
    })
}

/// Build a `reqwest::Client` with the configured TLS verification mode.
/// Used for real API calls; follows only same-host redirects so the API
/// key header cannot leak to a redirect-target host (see
/// [`same_host_redirect_policy`]).
pub fn build_tls_client(config: &TlsConfig) -> crate::error::Result<reqwest::Client> {
    let builder = reqwest::Client::builder()
        .connect_timeout(crate::http::CONNECT_TIMEOUT)
        .timeout(crate::http::REQUEST_TIMEOUT)
        .redirect(same_host_redirect_policy());
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
