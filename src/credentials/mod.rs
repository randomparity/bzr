//! Credential storage backends.
//!
//! Currently provides a single backend: the OS keychain, via the
//! [`keyring`] crate. Gated behind the `keyring` Cargo feature;
//! when disabled, a stub returns clear "unsupported" errors so the
//! binary still parses keyring-backed config entries.

#[cfg(feature = "keyring")]
pub mod keyring;

#[cfg(not(feature = "keyring"))]
#[path = "keyring_stub.rs"]
pub mod keyring;
