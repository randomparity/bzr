//! Stub keychain backend used when the `keyring` feature is disabled.
//!
//! Every function returns a clear error pointing the user at
//! `api_key_env` or a feature-enabled rebuild.

use crate::error::{BzrError, Result};

const UNSUPPORTED: &str = "this bzr build was compiled without keyring support; \
     rebuild with --features keyring or use api_key_env";

pub fn store(_service: &str, _account: &str, _secret: &str) -> Result<()> {
    Err(BzrError::Keyring(UNSUPPORTED.into()))
}

pub fn retrieve(_service: &str, _account: &str) -> Result<String> {
    Err(BzrError::Keyring(UNSUPPORTED.into()))
}

pub fn delete(_service: &str, _account: &str) -> Result<()> {
    Err(BzrError::Keyring(UNSUPPORTED.into()))
}
