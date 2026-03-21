use std::io::IsTerminal;
use std::process::ExitCode;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use bzr::cli::Cli;
use bzr::error::{self, BzrError};
use bzr::types::OutputFormat;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
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
            return exit_code(&e);
        }
    };

    if cli.quiet {
        suppress_stdout();
    }

    if let Err(e) = bzr::dispatch(&cli, format).await {
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
        return exit_code(&e);
    }

    ExitCode::SUCCESS
}

/// Convert a `BzrError` exit code (1-10) to a `std::process::ExitCode`.
fn exit_code(e: &BzrError) -> ExitCode {
    // All BzrError exit codes are in the range 1..=10.
    ExitCode::from(u8::try_from(e.exit_code()).unwrap_or(1))
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
    if let Some(out) = cli.output {
        return Ok(out);
    }
    if let Ok(val) = std::env::var("BZR_OUTPUT") {
        return val.parse().map_err(BzrError::InputValidation);
    }
    if std::io::stdout().is_terminal() {
        Ok(OutputFormat::Table)
    } else {
        Ok(OutputFormat::Json)
    }
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use super::*;
    use bzr::cli::Commands;

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
        cli.output = Some(OutputFormat::Json);
        let fmt = resolve_format(&cli).expect("should resolve");
        assert_eq!(fmt, OutputFormat::Json);
    }

    #[test]
    fn resolve_format_output_table() {
        let mut cli = base_cli(dummy_command());
        cli.output = Some(OutputFormat::Table);
        let fmt = resolve_format(&cli).expect("should resolve");
        assert_eq!(fmt, OutputFormat::Table);
    }

    #[test]
    fn resolve_format_json_overrides_output() {
        let mut cli = base_cli(dummy_command());
        cli.json = true;
        cli.output = Some(OutputFormat::Table);
        let fmt = resolve_format(&cli).expect("should resolve");
        assert_eq!(fmt, OutputFormat::Json);
    }
}
