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

pub(crate) mod batch;
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

/// Dispatch a parsed CLI to the appropriate command handler.
///
/// This is the shared dispatch logic used by both the binary (`main.rs`)
/// and integration tests, ensuring they exercise the same code paths.
pub async fn dispatch(
    cli: &cli::Cli,
    format: types::OutputFormat,
    w: &mut output::writers::Writers<'_>,
) -> error::Result<()> {
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
        cli::Commands::Whoami { action } => {
            let whoami_action = action.as_ref().unwrap_or(&cli::WhoamiAction::Show);
            commands::whoami::execute(whoami_action, server, format, api, w).await
        }
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
    }
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
