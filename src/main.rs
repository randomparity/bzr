mod auth;
mod cli;
mod client;
mod commands;
mod config;
mod error;
#[expect(clippy::print_stdout, clippy::expect_used)]
mod output;
mod xmlrpc;
mod xmlrpc_client;

use std::io::IsTerminal;

use clap::Parser;
use cli::{Cli, Commands};
use error::BzrError;
use output::OutputFormat;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else {
        // Level strings are compile-time constants; parse_lossy is safe.
        let level = match cli.verbose {
            0 => "bzr=warn",
            1 => "bzr=info",
            2 => "bzr=debug",
            _ => "bzr=trace",
        };
        EnvFilter::new(level)
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    // Disable colors when --no-color is set or stdout is not a TTY.
    if cli.no_color || !std::io::stdout().is_terminal() {
        colored::control::set_override(false);
    }

    let format = match resolve_format(&cli) {
        Ok(f) => f,
        Err(e) => {
            #[expect(clippy::print_stderr)]
            {
                eprintln!("error: {e}");
            }
            std::process::exit(e.exit_code());
        }
    };

    if let Err(e) = run(cli, format).await {
        #[expect(clippy::print_stderr)]
        {
            if format == OutputFormat::Json {
                let json_err = serde_json::json!({
                    "error": {
                        "type": e.error_type(),
                        "message": e.to_string(),
                        "exit_code": e.exit_code(),
                    }
                });
                eprintln!(
                    "{}",
                    serde_json::to_string(&json_err).unwrap_or_else(|_| {
                        r#"{"error":{"message":"serialization failed"}}"#.into()
                    }),
                );
            } else {
                eprintln!("error: {e}");
            }
        }
        std::process::exit(e.exit_code());
    }
}

/// Resolve output format from flags, env var, and TTY detection.
///
/// Precedence: `--json` > `--output` > `BZR_OUTPUT` env > auto-detect
/// (JSON when stdout is not a TTY, table otherwise).
fn resolve_format(cli: &Cli) -> error::Result<OutputFormat> {
    if cli.json {
        if cli.output.is_some() {
            tracing::warn!("--output ignored because --json takes precedence");
        }
        return Ok(OutputFormat::Json);
    }
    if let Some(ref out) = cli.output {
        return out.parse().map_err(BzrError::Other);
    }
    if let Ok(val) = std::env::var("BZR_OUTPUT") {
        return val.parse().map_err(BzrError::Other);
    }
    if std::io::stdout().is_terminal() {
        Ok(OutputFormat::Table)
    } else {
        Ok(OutputFormat::Json)
    }
}

async fn run(cli: Cli, format: OutputFormat) -> error::Result<()> {
    // When --quiet, suppress all stdout by discarding writes.
    if cli.quiet {
        suppress_stdout();
    }

    let api = cli.api;
    match &cli.command {
        Commands::Bug { action } => {
            commands::bug::execute(action, cli.server.as_deref(), format, api).await
        }
        Commands::Comment { action } => {
            commands::comment::execute(action, cli.server.as_deref(), format, api).await
        }
        Commands::Attachment { action } => {
            commands::attachment::execute(action, cli.server.as_deref(), format, api).await
        }
        Commands::Config { action } => commands::config_cmd::execute(action, format),
        Commands::Product { action } => {
            commands::product::execute(action, cli.server.as_deref(), format, api).await
        }
        Commands::Field { action } => {
            commands::field::execute(action, cli.server.as_deref(), format, api).await
        }
        Commands::User { action } => {
            commands::user::execute(action, cli.server.as_deref(), format, api).await
        }
        Commands::Group { action } => {
            commands::group::execute(action, cli.server.as_deref(), format, api).await
        }
        Commands::Whoami => commands::whoami::execute(cli.server.as_deref(), format, api).await,
        Commands::Server { action } => {
            commands::server::execute(action, cli.server.as_deref(), format, api).await
        }
        Commands::Classification { action } => {
            commands::classification::execute(action, cli.server.as_deref(), format, api).await
        }
        Commands::Component { action } => {
            commands::component::execute(action, cli.server.as_deref(), format, api).await
        }
    }
}

/// Redirect stdout to /dev/null for --quiet mode.
#[cfg(unix)]
fn suppress_stdout() {
    use std::os::unix::io::AsRawFd;
    // Safety: dup2 replaces stdout fd with /dev/null fd.
    // This is safe because we own the process and only do this once.
    if let Ok(devnull) = std::fs::File::open("/dev/null") {
        unsafe {
            libc::dup2(devnull.as_raw_fd(), libc::STDOUT_FILENO);
        }
    }
}

/// No-op on non-Unix platforms.
#[cfg(not(unix))]
fn suppress_stdout() {}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn base_cli(command: Commands) -> Cli {
        Cli {
            server: None,
            output: None,
            json: false,
            no_color: false,
            quiet: false,
            api: None,
            verbose: 0,
            command,
        }
    }

    fn dummy_command() -> Commands {
        Commands::Whoami
    }

    #[test]
    fn resolve_format_json_flag() {
        let mut cli = base_cli(dummy_command());
        cli.json = true;
        let fmt = resolve_format(&cli).expect("should resolve");
        assert_eq!(fmt, OutputFormat::Json);
    }

    #[test]
    fn resolve_format_output_json() {
        let mut cli = base_cli(dummy_command());
        cli.output = Some("json".into());
        let fmt = resolve_format(&cli).expect("should resolve");
        assert_eq!(fmt, OutputFormat::Json);
    }

    #[test]
    fn resolve_format_output_table() {
        let mut cli = base_cli(dummy_command());
        cli.output = Some("table".into());
        let fmt = resolve_format(&cli).expect("should resolve");
        assert_eq!(fmt, OutputFormat::Table);
    }

    #[test]
    fn resolve_format_invalid_output() {
        let mut cli = base_cli(dummy_command());
        cli.output = Some("xml".into());
        let err = resolve_format(&cli).unwrap_err();
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn resolve_format_json_overrides_output() {
        let mut cli = base_cli(dummy_command());
        cli.json = true;
        cli.output = Some("table".into());
        let fmt = resolve_format(&cli).expect("should resolve");
        assert_eq!(fmt, OutputFormat::Json);
    }
}
