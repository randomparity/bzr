//! Stub keychain backend used when the `keyring` feature is disabled.
//!
//! Every function returns a clear error pointing the user at
//! `api_key_env` or a feature-enabled rebuild.

use crate::error::{BzrError, Result};

const UNSUPPORTED: &str = "this bzr build was compiled without keyring support; \
     rebuild with --features keyring or use api_key_env";

// Mutation testing: this file is `cfg(not(feature = "keyring"))`;
// cargo-mutants runs with `--all-features`, so the bodies are never
// compiled into the test binary and the inline tests below never execute.
// Skip function-body mutations rather than carry a separate no-keyring
// test invocation.
#[cfg_attr(test, mutants::skip)]
pub fn store(_service: &str, _account: &str, _secret: &str) -> Result<()> {
    Err(BzrError::Keyring(UNSUPPORTED.into()))
}

#[cfg_attr(test, mutants::skip)]
pub fn retrieve(_service: &str, _account: &str) -> Result<String> {
    Err(BzrError::Keyring(UNSUPPORTED.into()))
}

#[cfg_attr(test, mutants::skip)]
pub fn delete(_service: &str, _account: &str) -> Result<()> {
    Err(BzrError::Keyring(UNSUPPORTED.into()))
}

#[cfg(test)]
#[path = "keyring_stub_tests.rs"]
mod tests;
