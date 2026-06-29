use std::io::{IsTerminal, Write};
use std::process::ExitCode;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use bzr::cli::Cli;
use bzr::error::{self, BzrError};
use bzr::types::OutputFormat;

// Mutation testing: `main` is the binary entry point. Defeating body-level
// mutations requires spawning the compiled binary (e.g. via assert_cmd or
// escargot) to observe exit codes and stderr. The pure helpers it delegates
// to (`tracing_filter_directive`, `format_dispatch_error`, `exit_code`,
// `resolve_format`, `suppress_stdout`) are unit-tested directly; the
// orchestration glue is not worth a new dev-dependency.
#[cfg_attr(test, mutants::skip)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    let filter =
        match tracing_filter_directive(cli.quiet, cli.verbose, std::env::var("RUST_LOG").is_ok()) {
            Some(directive) => EnvFilter::new(directive),
            None => EnvFilter::from_default_env(),
        };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    // Resolve format and colors BEFORE suppressing stdout, so that
    // is_terminal() sees the real fd and format selection is unaffected.
    if cli.no_color || !std::io::stdout().is_terminal() {
        colored::control::set_override(false);
    }

    let format = match resolve_format(&cli) {
        Ok(f) => f,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "error: {e}");
            return exit_code(&e);
        }
    };

    if cli.quiet {
        suppress_stdout();
    }

    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let mut out = stdout.lock();
    let mut err = stderr.lock();
    let mut writers = bzr::output::writers::Writers::new(&mut out, &mut err);

    if let Err(e) = bzr::dispatch(&cli, format, &mut writers).await {
        bzr::output::progress::error_event(
            cli.progress,
            writers.err,
            e.error_type(),
            e.exit_code(),
        );
        let _ = writeln!(writers.err, "{}", format_dispatch_error(&e, format));
        return exit_code(&e);
    }

    ExitCode::SUCCESS
}

/// Convert a `BzrError` exit code (1-13) to a `std::process::ExitCode`.
fn exit_code(e: &BzrError) -> ExitCode {
    // All BzrError exit codes are in the range 1..=13.
    ExitCode::from(u8::try_from(e.exit_code()).unwrap_or(1))
}

/// Render a dispatch error for the user.
///
/// JSON-family output (`json` and `ndjson`) renders a structured error object
/// with `type`, `message`, and `exit_code` fields — one compact line, so an
/// `ndjson` stream stays parseable. Pretty `--json` additionally carries the
/// top-level `schema_version` envelope key (present iff the format is `Json`,
/// matching the success path); `ndjson` stays bare. Table output renders the
/// conventional `error: …` prefix.
fn format_dispatch_error(err: &BzrError, format: OutputFormat) -> String {
    // Seed with the variant-specific structured keys, then write the three
    // universal keys LAST so a detail key can never clobber them.
    let mut error_object = err.structured_detail();
    error_object.insert("type".into(), err.error_type().into());
    error_object.insert("message".into(), err.to_string().into());
    error_object.insert("exit_code".into(), err.exit_code().into());
    let error_body = serde_json::Value::Object(error_object);
    let fallback = || r#"{"error":{"message":"serialization failed"}}"#.to_string();
    match format {
        OutputFormat::Json => serde_json::to_string(&serde_json::json!({
            "schema_version": bzr::output::SCHEMA_VERSION,
            "error": error_body,
        }))
        .unwrap_or_else(|_| fallback()),
        OutputFormat::Ndjson => serde_json::to_string(&serde_json::json!({ "error": error_body }))
            .unwrap_or_else(|_| fallback()),
        OutputFormat::Table => format!("error: {err}"),
    }
}

/// Select the tracing filter directive based on CLI flags.
///
/// Returns `None` when `RUST_LOG` should be used (caller falls back to
/// `EnvFilter::from_default_env()`).
fn tracing_filter_directive(quiet: bool, verbose: u8, rust_log_set: bool) -> Option<&'static str> {
    if quiet {
        return Some("off");
    }
    if rust_log_set {
        return None;
    }
    Some(match verbose {
        0 => "bzr=warn",
        1 => "bzr=info",
        2 => "bzr=debug",
        _ => "bzr=trace",
    })
}

/// Redirect stdout to the platform null device for --quiet mode.
#[cfg(unix)]
fn suppress_stdout() {
    use std::os::unix::io::AsRawFd;
    if let Ok(devnull) = std::fs::OpenOptions::new().write(true).open("/dev/null") {
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

// Mutation testing: dead code on the Linux test platform; cannot be observed.
#[cfg_attr(test, mutants::skip)]
#[cfg(windows)]
fn suppress_stdout() {
    use std::os::windows::io::IntoRawHandle;

    const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5; // -11i32 as u32
    extern "system" {
        fn SetStdHandle(nstdhandle: u32, hhandle: *mut std::ffi::c_void) -> i32;
    }

    if let Ok(nul) = std::fs::OpenOptions::new().write(true).open("NUL") {
        let handle = nul.into_raw_handle();
        // SAFETY: SetStdHandle replaces the process-wide stdout handle with
        // NUL. Rust's std::io::Stdout reads this handle, so all subsequent
        // println!/write! calls go to NUL. Called once at startup before any
        // other threads write to stdout. We intentionally leak `nul` (via
        // into_raw_handle) so the handle stays valid for the process lifetime.
        unsafe {
            SetStdHandle(STD_OUTPUT_HANDLE, handle);
        }
    }
}

// Mutation testing: dead code on the Linux test platform.
#[cfg_attr(test, mutants::skip)]
#[cfg(not(any(unix, windows)))]
fn suppress_stdout() {
    // No platform-specific suppression available; --quiet will only
    // suppress tracing output via the EnvFilter.
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
    if let Some(out) = cli.output {
        return Ok(out);
    }
    if let Ok(val) = std::env::var("BZR_OUTPUT") {
        return val.parse().map_err(BzrError::input);
    }
    if std::io::stdout().is_terminal() {
        Ok(OutputFormat::Table)
    } else {
        Ok(OutputFormat::Json)
    }
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
