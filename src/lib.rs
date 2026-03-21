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
pub mod error;
pub(crate) mod http;
#[expect(clippy::print_stdout, clippy::expect_used)]
pub(crate) mod output;
pub mod types;
pub(crate) mod xmlrpc;

/// Shared mutex for tests that modify the process-global `XDG_CONFIG_HOME` env var.
/// All such tests must acquire this lock to avoid racing with each other.
#[cfg(test)]
pub(crate) static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Shared XML-RPC test fixtures used across client/ and xmlrpc/ tests.
#[cfg(test)]
pub(crate) mod test_fixtures {
    /// Build a mock XML-RPC Bug.search response containing one bug.
    pub fn xmlrpc_bug_response(id: i64, summary: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
            <methodResponse><params><param><value><struct>
              <member><name>bugs</name><value><array><data>
                <value><struct>
                  <member><name>id</name><value><int>{id}</int></value></member>
                  <member><name>summary</name><value><string>{summary}</string></value></member>
                  <member><name>status</name><value><string>NEW</string></value></member>
                  <member><name>product</name><value><string>TestProduct</string></value></member>
                  <member><name>component</name><value><string>General</string></value></member>
                  <member><name>assigned_to</name><value><string>dev@example.com</string></value></member>
                  <member><name>priority</name><value><string>P1</string></value></member>
                  <member><name>severity</name><value><string>normal</string></value></member>
                  <member><name>keywords</name><value><array><data></data></array></value></member>
                  <member><name>blocks</name><value><array><data></data></array></value></member>
                  <member><name>depends_on</name><value><array><data></data></array></value></member>
                  <member><name>cc</name><value><array><data></data></array></value></member>
                </struct></value>
              </data></array></value></member>
            </struct></value></param></params></methodResponse>"#
        )
    }
}

/// Dispatch a parsed CLI to the appropriate command handler.
///
/// This is the shared dispatch logic used by both the binary (`main.rs`)
/// and integration tests, ensuring they exercise the same code paths.
pub async fn dispatch(cli: &cli::Cli, format: types::OutputFormat) -> error::Result<()> {
    let api = cli.api;
    let server = cli.server.as_deref();

    match &cli.command {
        cli::Commands::Bug { action } => commands::bug::execute(action, server, format, api).await,
        cli::Commands::Comment { action } => {
            commands::comment::execute(action, server, format, api).await
        }
        cli::Commands::Attachment { action } => {
            commands::attachment::execute(action, server, format, api).await
        }
        cli::Commands::Config { action } => commands::config::execute(action, format).await,
        cli::Commands::Product { action } => {
            commands::product::execute(action, server, format, api).await
        }
        cli::Commands::Field { action } => {
            commands::field::execute(action, server, format, api).await
        }
        cli::Commands::User { action } => {
            commands::user::execute(action, server, format, api).await
        }
        cli::Commands::Group { action } => {
            commands::group::execute(action, server, format, api).await
        }
        cli::Commands::Whoami => commands::whoami::execute(server, format, api).await,
        cli::Commands::Server { action } => {
            commands::server::execute(action, server, format, api).await
        }
        cli::Commands::Classification { action } => {
            commands::classification::execute(action, server, format, api).await
        }
        cli::Commands::Component { action } => {
            commands::component::execute(action, server, format, api).await
        }
    }
}
