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

/// Shared mutex for tests that modify the process-global `XDG_CONFIG_HOME` env var.
/// All such tests must acquire this lock to avoid racing with each other.
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Dispatch a parsed CLI to the appropriate command handler.
///
/// This is the shared dispatch logic used by both the binary (`main.rs`)
/// and integration tests, ensuring they exercise the same code paths.
pub async fn dispatch(cli: &cli::Cli, format: types::OutputFormat) -> error::Result<()> {
    if cli.quiet {
        suppress_stdout();
    }

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
        cli::Commands::Config { action } => commands::config_cmd::execute(action, format),
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

/// Redirect stdout to /dev/null for --quiet mode.
#[cfg(unix)]
fn suppress_stdout() {
    use std::os::unix::io::AsRawFd;
    if let Ok(devnull) = std::fs::File::open("/dev/null") {
        extern "C" {
            fn dup2(oldfd: std::ffi::c_int, newfd: std::ffi::c_int) -> std::ffi::c_int;
        }
        // SAFETY: dup2 replaces stdout fd with /dev/null. Called once at startup
        // before any other threads write to stdout.
        unsafe {
            dup2(devnull.as_raw_fd(), 1);
        }
    }
}

#[cfg(not(unix))]
fn suppress_stdout() {}
