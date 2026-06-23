//! Ad-hoc / inline server support for stateless invocations.
//!
//! The global `--server-url` / `--server-api-key-env` / `--server-email` flags
//! let a single command run against a Bugzilla instance that was never written
//! to any config file. The parsed definition is stashed here as process-global
//! state (set once in `dispatch`, before any client is built) and read by
//! `crate::commands::runtime::shared::connect_and_configure`, mirroring the pattern
//! used for `--dry-run` and `--yes`.
//!
//! Test callers that set this must hold `ENV_LOCK` and reset it to `None`
//! before releasing the lock, so the inline definition cannot leak into a
//! sibling test that expects config-based resolution.

use std::path::PathBuf;
use std::sync::RwLock;

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

static INLINE: RwLock<Option<InlineServer>> = RwLock::new(None);

/// Install (or clear) the inline server definition. Called once from
/// `dispatch` after argument parsing; `None` clears it.
pub fn set(inline: Option<InlineServer>) {
    if let Ok(mut guard) = INLINE.write() {
        *guard = inline;
    }
}

/// The inline server definition for this invocation, if `--server-url` was
/// given. Returns an owned clone so callers don't hold the lock.
#[must_use]
pub fn get() -> Option<InlineServer> {
    INLINE.read().ok().and_then(|guard| guard.clone())
}

#[cfg(test)]
#[path = "inline_server_tests.rs"]
mod tests;
