use std::io::IsTerminal;
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
            eprintln!("{}", format_dispatch_error(&e, format));
        }
        return exit_code(&e);
    }

    ExitCode::SUCCESS
}

/// Convert a `BzrError` exit code (1-12) to a `std::process::ExitCode`.
fn exit_code(e: &BzrError) -> ExitCode {
    // All BzrError exit codes are in the range 1..=12.
    ExitCode::from(u8::try_from(e.exit_code()).unwrap_or(1))
}

/// Render a dispatch error for the user.
///
/// JSON output renders a structured object with `type`, `message`, and
/// `exit_code` fields. Table output renders the conventional `error: …`
/// prefix.
fn format_dispatch_error(err: &BzrError, format: OutputFormat) -> String {
    if format == OutputFormat::Json {
        let json_err = serde_json::json!({
            "error": {
                "type": err.error_type(),
                "message": err.to_string(),
                "exit_code": err.exit_code(),
            }
        });
        serde_json::to_string(&json_err)
            .unwrap_or_else(|_| r#"{"error":{"message":"serialization failed"}}"#.into())
    } else {
        format!("error: {err}")
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
    use std::sync::{Mutex, OnceLock};

    use super::*;
    use bzr::cli::Commands;

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env lock poisoned")
    }

    fn with_bzr_output<T>(value: Option<&str>, f: impl FnOnce() -> T) -> T {
        let _guard = env_lock();
        let old = std::env::var("BZR_OUTPUT").ok();
        match value {
            Some(value) => unsafe { std::env::set_var("BZR_OUTPUT", value) },
            None => unsafe { std::env::remove_var("BZR_OUTPUT") },
        }

        let result = f();

        match old {
            Some(old) => unsafe { std::env::set_var("BZR_OUTPUT", old) },
            None => unsafe { std::env::remove_var("BZR_OUTPUT") },
        }

        result
    }

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
        Commands::Whoami {
            action: Some(bzr::cli::WhoamiAction::Show),
        }
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

    #[test]
    fn resolve_format_output_overrides_env() {
        with_bzr_output(Some("json"), || {
            let mut cli = base_cli(dummy_command());
            cli.output = Some(OutputFormat::Table);
            let fmt = resolve_format(&cli).expect("should resolve");
            assert_eq!(fmt, OutputFormat::Table);
        });
    }

    #[test]
    fn resolve_format_json_overrides_env() {
        with_bzr_output(Some("table"), || {
            let mut cli = base_cli(dummy_command());
            cli.json = true;
            let fmt = resolve_format(&cli).expect("should resolve");
            assert_eq!(fmt, OutputFormat::Json);
        });
    }

    #[test]
    fn resolve_format_env_json() {
        with_bzr_output(Some("json"), || {
            let cli = base_cli(dummy_command());
            let fmt = resolve_format(&cli).expect("should resolve");
            assert_eq!(fmt, OutputFormat::Json);
        });
    }

    #[test]
    fn resolve_format_invalid_env_returns_input_validation_error() {
        with_bzr_output(Some("xml"), || {
            let cli = base_cli(dummy_command());
            let err = resolve_format(&cli).expect_err("invalid format should fail");
            assert!(matches!(err, BzrError::InputValidation(_)));
        });
    }

    #[test]
    fn tracing_filter_quiet_returns_off() {
        assert_eq!(tracing_filter_directive(true, 0, false), Some("off"));
    }

    #[test]
    fn tracing_filter_quiet_overrides_verbose() {
        assert_eq!(tracing_filter_directive(true, 3, false), Some("off"));
    }

    #[test]
    fn tracing_filter_quiet_overrides_rust_log() {
        assert_eq!(tracing_filter_directive(true, 0, true), Some("off"));
    }

    #[test]
    fn tracing_filter_rust_log_defers() {
        assert_eq!(tracing_filter_directive(false, 0, true), None);
    }

    #[test]
    fn tracing_filter_default_warn() {
        assert_eq!(tracing_filter_directive(false, 0, false), Some("bzr=warn"));
    }

    #[test]
    fn tracing_filter_verbose_levels() {
        assert_eq!(tracing_filter_directive(false, 1, false), Some("bzr=info"));
        assert_eq!(tracing_filter_directive(false, 2, false), Some("bzr=debug"));
        assert_eq!(tracing_filter_directive(false, 3, false), Some("bzr=trace"));
        assert_eq!(
            tracing_filter_directive(false, 10, false),
            Some("bzr=trace")
        );
    }

    #[test]
    fn resolve_format_falls_back_to_tty_detection() {
        // With no flags and no BZR_OUTPUT env var, resolve_format() reaches
        // the is_terminal() branch. Output depends on whether stdout is a TTY
        // when tests run, but either way the call must succeed.
        with_bzr_output(None, || {
            let cli = base_cli(dummy_command());
            let fmt = resolve_format(&cli).expect("should resolve");
            assert!(matches!(fmt, OutputFormat::Json | OutputFormat::Table));
        });
    }

    #[test]
    fn exit_code_maps_known_error_to_exit_code() {
        // Spot-check that exit_code() produces an ExitCode for an in-range value.
        // The ExitCode type is opaque, so we only verify the call succeeds.
        let err = BzrError::InputValidation("bad input".into());
        let code = exit_code(&err);
        // ExitCode does not implement PartialEq; compare via Debug instead.
        let rendered = format!("{code:?}");
        assert!(
            rendered.contains(&err.exit_code().to_string()),
            "expected ExitCode debug to include {}, got {rendered}",
            err.exit_code()
        );
    }

    #[test]
    fn exit_code_maps_other_variant() {
        // Cover the non-validation branch as well.
        let err = BzrError::Other("boom".into());
        let code = exit_code(&err);
        let rendered = format!("{code:?}");
        assert!(rendered.contains(&err.exit_code().to_string()));
    }

    #[test]
    fn format_dispatch_error_renders_json() {
        let err = BzrError::Config("bad config".into());
        let out = format_dispatch_error(&err, OutputFormat::Json);
        let parsed: serde_json::Value =
            serde_json::from_str(&out).expect("output must be valid JSON");
        let inner = parsed.get("error").expect("has error key");
        assert_eq!(inner["type"], err.error_type());
        assert_eq!(inner["exit_code"], err.exit_code());
        assert!(
            inner["message"]
                .as_str()
                .expect("message is string")
                .contains("bad config"),
            "message should contain underlying error: {out}"
        );
    }

    #[test]
    fn format_dispatch_error_renders_table() {
        let err = BzrError::Config("bad config".into());
        let out = format_dispatch_error(&err, OutputFormat::Table);
        assert!(out.starts_with("error:"), "got {out:?}");
        assert!(out.contains("bad config"), "got {out:?}");
    }

    #[test]
    fn format_dispatch_error_table_for_other_error() {
        let err = BzrError::Other("kaboom".into());
        let out = format_dispatch_error(&err, OutputFormat::Table);
        assert!(out.contains("kaboom"));
    }

    /// After `suppress_stdout()` runs, writes to fd 1 must NOT reach
    /// whatever fd 1 pointed at before the call. We redirect fd 1 to a
    /// temp file, invoke `suppress_stdout()`, write a marker, then
    /// inspect the temp file: if `suppress_stdout` is a no-op the marker
    /// lands in our file; if it works the marker goes to /dev/null.
    #[cfg(unix)]
    #[test]
    fn suppress_stdout_redirects_fd1_to_devnull() {
        use std::io::{Read, Seek};
        use std::os::unix::io::AsRawFd;

        extern "C" {
            fn dup(fd: std::ffi::c_int) -> std::ffi::c_int;
            fn dup2(oldfd: std::ffi::c_int, newfd: std::ffi::c_int) -> std::ffi::c_int;
            fn close(fd: std::ffi::c_int) -> std::ffi::c_int;
            fn write(fd: std::ffi::c_int, buf: *const u8, count: usize) -> isize;
        }

        let _guard = env_lock();
        let tmp = tempfile::NamedTempFile::new().expect("tmpfile");
        let tmp_fd = tmp.as_file().as_raw_fd();

        // SAFETY: dup() returns a duplicate of fd 1; we restore it below.
        let saved = unsafe { dup(1) };
        assert!(saved >= 0, "dup(1) failed");
        // SAFETY: dup2 redirects fd 1 to the temp file.
        unsafe {
            dup2(tmp_fd, 1);
        }

        suppress_stdout();

        let marker = b"MARKER_AFTER_SUPPRESS";
        // SAFETY: writing a small buffer to fd 1 (now /dev/null after a
        // successful suppress_stdout, or our temp file if the call was
        // mutated to no-op).
        unsafe {
            write(1, marker.as_ptr(), marker.len());
        }

        // Restore fd 1 BEFORE reading the temp file so cargo's harness
        // and any later test gets a working stdout.
        // SAFETY: dup2/close on valid fds.
        unsafe {
            dup2(saved, 1);
            close(saved);
        }

        let mut captured = Vec::new();
        let mut f = tmp.reopen().expect("reopen");
        f.seek(std::io::SeekFrom::Start(0)).expect("seek");
        f.read_to_end(&mut captured).expect("read");

        let captured_str = String::from_utf8_lossy(&captured);
        assert!(
            !captured_str.contains("MARKER_AFTER_SUPPRESS"),
            "suppress_stdout did not redirect fd 1; marker reached temp file: {captured_str:?}"
        );
    }
}
