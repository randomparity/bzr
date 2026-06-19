//! Library crate for bzr — exposes modules for integration testing.
//!
//! The primary entry point is the binary crate (`main.rs`). This library
//! exists so that integration tests in `tests/` can access internal modules.
#![expect(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::module_name_repetitions,
    reason = "public API is for integration tests, not external consumers"
)]

pub mod cli;
pub mod client;
pub mod commands;
pub mod config;
pub mod credentials;
pub mod error;
pub(crate) mod field_aliases;
pub(crate) mod http;
#[expect(clippy::expect_used)]
pub mod output;
pub(crate) mod tls;
pub mod types;
pub mod url_parser;
pub mod validation;
pub mod xmlrpc;

/// Fuzz-only entry points. Gated behind `cfg(fuzzing)` so they expose the
/// otherwise crate-private DER walkers to the `fuzz/` harness without
/// widening the public API in normal builds.
#[cfg(fuzzing)]
pub mod fuzz {
    /// Drive the best-effort issuer DER walkers on arbitrary bytes. Must
    /// terminate without panicking for any input.
    pub fn extract_issuer(data: &[u8]) {
        let _ = crate::tls::verifier::extract_issuer_der(data);
        let _ = crate::tls::verifier::extract_issuer_dn(data);
    }
}

/// Dispatch a parsed CLI to the appropriate command handler.
///
/// This is the shared dispatch logic used by both the binary (`main.rs`)
/// and integration tests, ensuring they exercise the same code paths.
pub async fn dispatch(
    cli: &cli::Cli,
    format: types::OutputFormat,
    w: &mut output::writers::Writers<'_>,
) -> error::Result<()> {
    apply_network_tuning(cli);

    let api = cli.api;
    let server = cli.server.as_deref();

    match &cli.command {
        cli::Commands::Bug { action } => {
            commands::bug::execute(action, server, format, api, w).await
        }
        cli::Commands::Comment { action } => {
            commands::comment::execute(action, server, format, api, w).await
        }
        cli::Commands::Attachment { action } => {
            commands::attachment::execute(action, server, format, api, w).await
        }
        cli::Commands::Config { action } => {
            commands::config::execute(action, server, format, api, w).await
        }
        cli::Commands::Product { action } => {
            commands::product::execute(action, server, format, api, w).await
        }
        cli::Commands::Field { action } => {
            commands::field::execute(action, server, format, api, w).await
        }
        cli::Commands::User { action } => {
            commands::user::execute(action, server, format, api, w).await
        }
        cli::Commands::Group { action } => {
            commands::group::execute(action, server, format, api, w).await
        }
        cli::Commands::Whoami => commands::whoami::execute(server, format, api, w).await,
        cli::Commands::Server { action } => {
            commands::server::execute(action, server, format, api, w).await
        }
        cli::Commands::Classification { action } => {
            commands::classification::execute(action, server, format, api, w).await
        }
        cli::Commands::Component { action } => {
            commands::component::execute(action, server, format, api, w).await
        }
        cli::Commands::Template { action } => {
            commands::template::execute(action, server, format, api, w).await
        }
        cli::Commands::Query { action } => {
            commands::query::execute(action, server, format, api, w).await
        }
        cli::Commands::Completion { shell } => commands::completion::execute(*shell, w),
    }
}

/// Install process-wide network tuning from the global flags before any client
/// is built: the request timeout (`--timeout`, falling back to `BZR_TIMEOUT`)
/// and the transient-retry budget (`--retry`). An invalid `BZR_TIMEOUT` is
/// ignored with a warning so the built-in default stands.
///
/// This mutates process-global state, so `dispatch` is not safe to call
/// concurrently from one process with differing `--timeout`/`--retry` values
/// (the CLI runs a single dispatch per process, so this is not a concern in
/// practice; tests exercise per-client retry overrides instead).
fn apply_network_tuning(cli: &cli::Cli) {
    let env_timeout = std::env::var("BZR_TIMEOUT").ok();
    if cli.timeout.is_none() {
        if let Some(raw) = &env_timeout {
            if http::resolve_timeout_secs(None, Some(raw)).is_none() {
                tracing::warn!(
                    "ignoring invalid BZR_TIMEOUT={raw:?} (expected a positive integer)"
                );
            }
        }
    }
    http::set_request_timeout_secs(http::resolve_timeout_secs(
        cli.timeout,
        env_timeout.as_deref(),
    ));
    http::set_retry_max(cli.retry.unwrap_or(0));
}

/// Shared mutex for tests that modify the process-global `XDG_CONFIG_HOME` env var.
/// All such tests must acquire this lock to avoid racing with each other.
#[cfg(any(test, feature = "test-helpers"))]
pub static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Shared test helpers used by both unit tests and integration tests.
#[cfg(any(test, feature = "test-helpers"))]
pub mod test_helpers;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
