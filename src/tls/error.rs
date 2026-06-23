/// Check if an error message string contains TLS-related keywords.
pub(crate) fn looks_like_tls_error(msg: &str) -> bool {
    let lower = msg.to_ascii_lowercase();
    lower.contains("cert") || lower.contains("ssl") || lower.contains("tls")
}

/// Pure predicate underlying [`is_tls_cert_error`], split out so the
/// connect-and-TLS-keyword logic can be unit tested without a live
/// `reqwest::Error` (which has no public constructor). Also called directly by
/// `format_http_error` to avoid recomputing the error chain.
pub(crate) fn is_connect_tls_error(is_connect: bool, error_chain: &str) -> bool {
    is_connect && looks_like_tls_error(error_chain)
}

/// Check if a reqwest error looks like a TLS certificate verification failure.
pub(crate) fn is_tls_cert_error(err: &reqwest::Error) -> bool {
    is_connect_tls_error(err.is_connect(), &crate::error::format_error_chain(err))
}

/// Hint text appended to TLS certificate errors.
pub(crate) const TLS_HINT: &str =
    "\n  hint: to trust this server's certificate, re-run interactively,\n    \
     or pre-pin with:  bzr config set-server <NAME> --tls-pin-now\n    \
     or provide a CA:  bzr config set-server <NAME> --tls-ca-cert <PATH>\n    \
     or skip verification: bzr config set-server <NAME> --tls-insecure";

/// Append a TLS trust hint to a message when a certificate error is detected.
pub(crate) fn tls_hint(base_msg: &str, err: &reqwest::Error) -> String {
    let mut msg = base_msg.to_string();
    if is_tls_cert_error(err) {
        msg.push_str(TLS_HINT);
    }
    msg
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
