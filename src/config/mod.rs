mod model;
mod store;

pub use model::{
    Config, CredentialSource, CredentialSourceKind, KeyringAccount, KeyringRef, ServerConfig,
};

#[cfg(all(test, unix))]
use store::fsync_parent_dir;
#[cfg(test)]
use store::set_fail_after_temp;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
