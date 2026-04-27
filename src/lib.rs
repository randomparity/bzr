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
pub(crate) mod http;
#[expect(clippy::print_stdout, clippy::expect_used)]
pub(crate) mod output;
pub mod types;
pub mod url_parser;
pub(crate) mod xmlrpc;

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
        cli::Commands::Config { action } => {
            commands::config::execute(action, server, format, api).await
        }
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
        cli::Commands::Whoami { action } => {
            let whoami_action = action.as_ref().unwrap_or(&cli::WhoamiAction::Show);
            commands::whoami::execute(whoami_action, server, format, api).await
        }
        cli::Commands::Server { action } => {
            commands::server::execute(action, server, format, api).await
        }
        cli::Commands::Classification { action } => {
            commands::classification::execute(action, server, format, api).await
        }
        cli::Commands::Component { action } => {
            commands::component::execute(action, server, format, api).await
        }
        cli::Commands::Template { action } => {
            commands::template::execute(action, server, format, api).await
        }
        cli::Commands::Query { action } => {
            commands::query::execute(action, server, format, api).await
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
#[expect(clippy::unwrap_used)]
mod tests {
    use clap::Parser;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    use super::*;
    use crate::test_helpers::{capture_stdout, extract_json, setup_test_env};
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn dispatch_whoami_defaults_to_show_action() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/whoami"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1,
                "name": "admin@example.com",
                "real_name": "Admin User"
            })))
            .expect(1)
            .mount(&mock)
            .await;

        let cli =
            cli::Cli::try_parse_from(["bzr", "--server", "test", "--json", "whoami"]).unwrap();
        let (result, output) = capture_stdout(dispatch(&cli, OutputFormat::Json)).await;
        assert!(result.is_ok(), "dispatch failed: {result:?}");

        let parsed = extract_json(&output);
        assert_eq!(parsed["name"], "admin@example.com");
    }

    #[tokio::test]
    async fn dispatch_routes_local_query_commands() {
        let (_lock, _mock, _tmp) = setup_test_env().await;

        let cli = cli::Cli::try_parse_from([
            "bzr",
            "--json",
            "query",
            "save",
            "firefox-new",
            "--product",
            "Firefox",
        ])
        .unwrap();
        let (result, output) = capture_stdout(dispatch(&cli, OutputFormat::Json)).await;
        assert!(result.is_ok(), "dispatch failed: {result:?}");

        let parsed = extract_json(&output);
        assert_eq!(parsed["name"], "firefox-new");
        assert_eq!(parsed["action"], "saved");
    }
}
