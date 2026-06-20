#![expect(clippy::expect_used)]

use super::*;
use std::sync::{Mutex, OnceLock};

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
        config: None,
        no_color: false,
        quiet: false,
        api: None,
        timeout: None,
        retry: None,
        dry_run: false,
        yes: false,
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
fn exit_code_maps_highest_tls_code() {
    // Guards the upper bound documented on `exit_code`: the TLS variants
    // return 13, the largest code, and must survive the `u8::try_from`
    // conversion without collapsing to the `unwrap_or(1)` fallback.
    let err = BzrError::PinMismatch {
        server: "example.com".into(),
        expected: "sha256//old".into(),
        actual: "sha256//new".into(),
    };
    assert_eq!(err.exit_code(), 13);
    let code = exit_code(&err);
    let rendered = format!("{code:?}");
    assert!(
        rendered.contains("13"),
        "expected ExitCode debug to include 13, got {rendered}"
    );
}

#[test]
fn format_dispatch_error_renders_json() {
    let err = BzrError::Config("bad config".into());
    let out = format_dispatch_error(&err, OutputFormat::Json);
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("output must be valid JSON");
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
