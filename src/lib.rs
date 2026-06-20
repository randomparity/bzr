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
    ensure_dry_run_supported(cli)?;
    commands::dry_run::set(cli.dry_run);
    commands::confirm::set_yes(cli.yes);
    commands::inline_server::set(resolve_inline_server(cli));

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

/// Build the inline server definition from the global `--server-url` flags, or
/// `None` when no inline server was requested. Returns `Some` only when both
/// the URL and its API-key env var are present; clap's `requires` guarantees
/// they come as a pair, but this is robust if a `Cli` is constructed directly
/// (as integration tests do).
fn resolve_inline_server(cli: &cli::Cli) -> Option<commands::inline_server::InlineServer> {
    cli.server_url
        .as_ref()
        .zip(cli.server_api_key_env.as_ref())
        .map(|(url, api_key_env)| commands::inline_server::InlineServer {
            url: url.clone(),
            api_key_env: api_key_env.clone(),
            email: cli.server_email.clone(),
        })
}

/// Reject `--dry-run` on commands that don't honor it.
///
/// `--dry-run` is a global flag (so it can appear after any subcommand), but
/// only the bug mutations preview without writing. Allowing it elsewhere would
/// silently ignore it — e.g. `bzr comment add --dry-run` would still post the
/// comment. Fail fast (exit 7) instead of writing when a preview was asked for.
fn ensure_dry_run_supported(cli: &cli::Cli) -> error::Result<()> {
    if !cli.dry_run {
        return Ok(());
    }
    if let cli::Commands::Bug { action } = &cli.command {
        if commands::bug::is_dry_runnable(action) {
            return Ok(());
        }
    }
    Err(error::BzrError::InputValidation(
        "--dry-run is only supported for bug create, update, clone, resolve, close, reopen, and dup"
            .into(),
    ))
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
