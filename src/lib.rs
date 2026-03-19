//! Library crate for bzr — exposes modules for integration testing.
//!
//! The primary entry point is the binary crate (`main.rs`). This library
//! exists so that integration tests in `tests/` can access internal modules.
#![expect(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::module_name_repetitions,
    reason = "public API is for integration tests, not external consumers"
)]

pub mod auth;
pub mod cli;
pub mod client;
pub mod commands;
pub mod config;
pub mod error;
pub(crate) mod http;
#[expect(clippy::print_stdout, clippy::expect_used)]
pub mod output;
pub mod types;
pub(crate) mod xmlrpc;
pub(crate) mod xmlrpc_client;
