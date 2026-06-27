//! Ad-hoc / inline server support for stateless invocations.
//!
//! The global `--server-url` / `--server-api-key-env` / `--server-email` flags
//! let a single command run against a Bugzilla instance that was never written
//! to any config file. The parsed definition is carried by
//! [`crate::commands::runtime::invocation::CommandContext`] and consumed by
//! `crate::commands::runtime::shared::connect_and_configure`.

use std::path::PathBuf;

/// Synthetic server name for an inline server. Parenthesized so it can never
/// collide with a real (TOML-table) config server name; shown in error
/// messages and any TLS prompt.
pub const INLINE_SERVER_NAME: &str = "(inline)";

/// TLS trust options for an ad-hoc server. These mirror the named-server TLS
/// controls that make sense without persistent config.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InlineTlsOptions {
    pub insecure: bool,
    pub ca_cert_path: Option<PathBuf>,
    pub pin_sha256: Option<String>,
    pub pin_now: bool,
}

/// An ad-hoc server defined entirely on the command line. The API key env var
/// is optional for public read-only commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineServer {
    pub url: String,
    pub api_key_env: Option<String>,
    pub email: Option<String>,
    pub tls: InlineTlsOptions,
}

#[cfg(test)]
#[path = "inline_server_tests.rs"]
mod tests;
