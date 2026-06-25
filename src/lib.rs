//! Library crate backing the `bzr` command-line tool.
//!
//! `bzr` ships as a CLI binary (`main.rs`); this library is the binary's
//! implementation, factored into modules so the binary and tests exercise
//! exactly the same code paths.
//!
//! ## Public boundary
//!
//! The intended entry point is [`dispatch`], which runs a parsed
//! [`cli::Cli`]. Public modules support CLI parsing, configuration, output,
//! and error reporting around that entry point. Implementation modules are
//! crate-private in normal builds; the `test-helpers` feature widens selected
//! modules for the integration-test harness only. Genuinely test-only items
//! (`ENV_LOCK`, `test_helpers`) are gated behind `cfg(test)` / the
//! `test-helpers` feature and never compile into a normal release build.
#![expect(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::module_name_repetitions,
    reason = "internal surface for the binary and integration tests, not external consumers"
)]

pub(crate) mod bugzilla_auth;
pub mod cli;
#[cfg(not(feature = "test-helpers"))]
pub(crate) mod client;
#[cfg(feature = "test-helpers")]
pub mod client;
#[cfg(not(feature = "test-helpers"))]
pub(crate) mod commands;
#[cfg(feature = "test-helpers")]
pub mod commands;
#[cfg(not(feature = "test-helpers"))]
pub(crate) mod config;
#[cfg(feature = "test-helpers")]
pub mod config;
#[cfg(not(feature = "test-helpers"))]
pub(crate) mod credentials;
#[cfg(feature = "test-helpers")]
pub mod credentials;
pub mod error;
pub(crate) mod http;
#[expect(clippy::expect_used)]
pub mod output;
pub(crate) mod tls;
pub mod types;
#[cfg(not(feature = "test-helpers"))]
pub(crate) mod validation;
#[cfg(feature = "test-helpers")]
pub mod validation;
#[cfg(not(feature = "test-helpers"))]
pub(crate) mod xmlrpc;
#[cfg(feature = "test-helpers")]
pub mod xmlrpc;

/// Fuzz-only entry points. Gated behind `cfg(fuzzing)` so they expose the
/// otherwise crate-private DER walkers to the `fuzz/` harness without
/// widening the public API in normal builds.
#[cfg(fuzzing)]
pub mod fuzz {
    use crate::config::Config;

    /// Drive the best-effort issuer DER walkers on arbitrary bytes. Must
    /// terminate without panicking for any input.
    pub fn extract_issuer(data: &[u8]) {
        let _ = crate::tls::verifier::extract_issuer_der(data);
        let _ = crate::tls::verifier::extract_issuer_dn(data);
    }

    /// Drive Bugzilla flag parsing on arbitrary string lists.
    pub fn parse_flags(input: &[String]) {
        let _ = crate::commands::runtime::flags::parse_flags(input);
    }

    /// Drive Bugzilla URL import parsing on arbitrary strings.
    ///
    /// Builds the fixed test config internally so the fuzz harness does not
    /// need to reach into the crate-internal `config` module.
    pub fn parse_bugzilla_url(data: &str) {
        let config: Config = toml::from_str(
            "default_server = \"test\"\n\n\
             [servers.test]\n\
             url = \"https://bugzilla.example.com\"\n\
             api_key = \"dummy\"\n",
        )
        .expect("static fuzz config is always valid");
        let _ = crate::commands::runtime::url_parser::parse_bugzilla_url(data, &config);
    }

    /// Drive XML-RPC response parsing on arbitrary strings.
    pub fn parse_xmlrpc_response(data: &str) {
        let _ = crate::xmlrpc::protocol::parse_response(data);
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
    let capabilities = command_capabilities(&cli.command);
    let ctx = build_command_context(cli, format, capabilities);
    ensure_dispatch_allowed(cli, capabilities)?;

    match &cli.command {
        cli::Commands::Bug { action } => commands::bug::execute(action, &ctx, w).await,
        cli::Commands::Comment { action } => commands::comment::execute(action, &ctx, w).await,
        cli::Commands::Attachment { action } => {
            commands::attachment::execute(action, &ctx, w).await
        }
        cli::Commands::Config { action } => commands::config::execute(action, &ctx, w).await,
        cli::Commands::Product { action } => commands::product::execute(action, &ctx, w).await,
        cli::Commands::Field { action } => commands::field::execute(action, &ctx, w).await,
        cli::Commands::User { action } => commands::user::execute(action, &ctx, w).await,
        cli::Commands::Group { action } => commands::group::execute(action, &ctx, w).await,
        cli::Commands::Whoami => commands::whoami::execute(&ctx, w).await,
        cli::Commands::Server { action } => commands::server::execute(action, &ctx, w).await,
        cli::Commands::Classification { action } => {
            commands::classification::execute(action, &ctx, w).await
        }
        cli::Commands::Component { action } => commands::component::execute(action, &ctx, w).await,
        cli::Commands::Template { action } => commands::template::execute(action, &ctx, w).await,
        cli::Commands::Query { action } => commands::query::execute(action, &ctx, w).await,
        cli::Commands::Completion { shell } => commands::completion::execute(*shell, &ctx, w).await,
        cli::Commands::Schema { name } => commands::schema::execute(name.as_deref(), &ctx, w).await,
    }
}

fn ensure_dispatch_allowed(
    cli: &cli::Cli,
    capabilities: commands::runtime::capabilities::CommandCapabilities,
) -> error::Result<()> {
    ensure_dry_run_supported(cli, capabilities)
}

/// Build the explicit command context from global CLI flags.
fn build_command_context(
    cli: &cli::Cli,
    format: types::OutputFormat,
    capabilities: commands::runtime::capabilities::CommandCapabilities,
) -> commands::runtime::context::CommandContext {
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
    let request_timeout = http::resolve_timeout_secs(cli.timeout, env_timeout.as_deref())
        .map_or(http::REQUEST_TIMEOUT, std::time::Duration::from_secs);
    commands::runtime::context::CommandContext::new(cli.server.as_deref(), format, cli.api)
        .with_dry_run(cli.dry_run)
        .with_assume_yes(cli.yes)
        .with_inline_server(resolve_inline_server(cli))
        .with_config_path_override(cli.config.clone())
        .with_request_timeout(request_timeout)
        .with_retry_max(cli.retry.unwrap_or(0))
        .with_credential_requirement(capabilities.credential_requirement())
}

/// Build the inline server definition from the global `--server-url` flags, or
/// `None` when no inline server was requested. The API-key env var is optional
/// for public read-only commands.
fn resolve_inline_server(cli: &cli::Cli) -> Option<commands::runtime::inline_server::InlineServer> {
    cli.server_url
        .as_ref()
        .map(|url| commands::runtime::inline_server::InlineServer {
            url: url.clone(),
            api_key_env: cli.server_api_key_env.clone(),
            email: cli.server_email.clone(),
            tls: commands::runtime::inline_server::InlineTlsOptions {
                insecure: cli.server_tls_insecure,
                ca_cert_path: cli.server_tls_ca_cert.clone(),
                pin_sha256: cli.server_tls_pin_sha256.clone(),
                pin_now: cli.server_tls_pin_now,
            },
        })
}

/// Reject `--dry-run` on commands that don't honor it.
///
/// `--dry-run` is a global flag (so it can appear after any subcommand), but
/// only selected mutations preview without writing. Allowing it elsewhere would
/// silently ignore it — e.g. `bzr comment add --dry-run` would still post the
/// comment. Fail fast (exit 7) instead of writing when a preview was asked for.
fn ensure_dry_run_supported(
    cli: &cli::Cli,
    capabilities: commands::runtime::capabilities::CommandCapabilities,
) -> error::Result<()> {
    if !cli.dry_run {
        return Ok(());
    }
    if capabilities.supports_dry_run() {
        return Ok(());
    }
    Err(error::BzrError::InputValidation(
        "--dry-run is only supported for bug create/update/clone/resolve/close/reopen/dup, \
         product create/update, component create/update, user create/update, and group create/update"
            .into(),
    ))
}

fn command_capabilities(
    command: &cli::Commands,
) -> commands::runtime::capabilities::CommandCapabilities {
    match command {
        cli::Commands::Bug { action } => commands::bug::capabilities(action),
        cli::Commands::Comment { action } => commands::comment::capabilities(action),
        cli::Commands::Attachment { action } => commands::attachment::capabilities(action),
        cli::Commands::Product { action } => commands::product::capabilities(action),
        cli::Commands::Component { action } => commands::component::capabilities(action),
        cli::Commands::User { action } => commands::user::capabilities(action),
        cli::Commands::Group { action } => commands::group::capabilities(action),
        cli::Commands::Whoami => {
            commands::runtime::capabilities::CommandCapabilities::authenticated("whoami")
        }
        cli::Commands::Config { .. }
        | cli::Commands::Field { .. }
        | cli::Commands::Server { .. }
        | cli::Commands::Classification { .. }
        | cli::Commands::Template { .. }
        | cli::Commands::Query { .. }
        | cli::Commands::Completion { .. }
        | cli::Commands::Schema { .. } => {
            commands::runtime::capabilities::CommandCapabilities::anonymous()
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
