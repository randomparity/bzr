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

/// Parse a `Cli` with all flags at their defaults, for the `resolve_format`
/// tests that only mutate public flag fields. Uses the real parser (rather
/// than a struct literal) because the `command` field is crate-internal.
fn base_cli() -> Cli {
    use clap::Parser;
    Cli::try_parse_from(["bzr", "whoami"]).expect("base cli parses")
}

#[test]
fn resolve_format_json_flag() {
    let mut cli = base_cli();
    cli.json = true;
    let fmt = resolve_format(&cli).expect("should resolve");
    assert_eq!(fmt, OutputFormat::Json);
}

#[test]
fn resolve_format_output_json() {
    let mut cli = base_cli();
    cli.output = Some(OutputFormat::Json);
    let fmt = resolve_format(&cli).expect("should resolve");
    assert_eq!(fmt, OutputFormat::Json);
}

#[test]
fn resolve_format_output_table() {
    let mut cli = base_cli();
    cli.output = Some(OutputFormat::Table);
    let fmt = resolve_format(&cli).expect("should resolve");
    assert_eq!(fmt, OutputFormat::Table);
}

#[test]
fn resolve_format_json_overrides_output() {
    let mut cli = base_cli();
    cli.json = true;
    cli.output = Some(OutputFormat::Table);
    let fmt = resolve_format(&cli).expect("should resolve");
    assert_eq!(fmt, OutputFormat::Json);
}

#[test]
fn resolve_format_output_overrides_env() {
    with_bzr_output(Some("json"), || {
        let mut cli = base_cli();
        cli.output = Some(OutputFormat::Table);
        let fmt = resolve_format(&cli).expect("should resolve");
        assert_eq!(fmt, OutputFormat::Table);
    });
}

#[test]
fn resolve_format_json_overrides_env() {
    with_bzr_output(Some("table"), || {
        let mut cli = base_cli();
        cli.json = true;
        let fmt = resolve_format(&cli).expect("should resolve");
        assert_eq!(fmt, OutputFormat::Json);
    });
}

#[test]
fn resolve_format_env_json() {
    with_bzr_output(Some("json"), || {
        let cli = base_cli();
        let fmt = resolve_format(&cli).expect("should resolve");
        assert_eq!(fmt, OutputFormat::Json);
    });
}

#[test]
fn resolve_format_invalid_env_returns_input_validation_error() {
    with_bzr_output(Some("xml"), || {
        let cli = base_cli();
        let err = resolve_format(&cli).expect_err("invalid format should fail");
        assert!(matches!(err, BzrError::InputValidation { .. }));
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
        let cli = base_cli();
        let fmt = resolve_format(&cli).expect("should resolve");
        assert!(matches!(fmt, OutputFormat::Json | OutputFormat::Table));
    });
}

#[test]
fn table_width_for_format_skips_resolver_for_json_family() {
    use std::cell::Cell;

    let called = Cell::new(false);
    let width = table_width_for_format(OutputFormat::Json, || {
        called.set(true);
        bzr::output::writers::resolve_table_width(None, Some(80))
    });

    assert_eq!(width, bzr::output::writers::TableWidth::default());
    assert!(!called.get());
}

#[test]
fn table_width_for_format_resolves_table_output() {
    let width = table_width_for_format(OutputFormat::Table, || {
        bzr::output::writers::resolve_table_width(None, Some(80))
    });

    assert_eq!(
        width,
        bzr::output::writers::resolve_table_width(None, Some(80))
    );
}

#[test]
fn exit_code_maps_known_error_to_exit_code() {
    // Spot-check that exit_code() produces an ExitCode for an in-range value.
    // The ExitCode type is opaque, so we only verify the call succeeds.
    let err = BzrError::input("bad input".into());
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
fn exit_code_maps_non_validation_variant() {
    // Cover the non-validation branch as well.
    let err = BzrError::Auth("boom".into());
    let code = exit_code(&err);
    let rendered = format!("{code:?}");
    assert!(rendered.contains(&err.exit_code().to_string()));
}

#[test]
fn exit_code_maps_highest_code() {
    let err = BzrError::MidAirCollision {
        id: 123,
        expected: "2026-06-22T01:02:03Z".into(),
        actual: "2026-06-22T01:02:04Z".into(),
    };
    assert_eq!(err.exit_code(), 14);
    let code = exit_code(&err);
    let rendered = format!("{code:?}");
    assert!(
        rendered.contains("14"),
        "expected ExitCode debug to include 14, got {rendered}"
    );
}

#[test]
fn format_dispatch_error_renders_json() {
    let err = BzrError::Config("bad config".into());
    let out = format_dispatch_error(&err, OutputFormat::Json);
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("output must be valid JSON");
    assert_eq!(
        parsed["schema_version"],
        bzr::output::SCHEMA_VERSION,
        "--json error output must carry the schema_version envelope: {out}"
    );
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
fn format_dispatch_error_ndjson_renders_compact_json_object() {
    // Under --output ndjson the error must stay a parseable JSON object (one
    // compact line), not degrade to the human `error: …` table form.
    let err = BzrError::Config("bad config".into());
    let out = format_dispatch_error(&err, OutputFormat::Ndjson);
    assert!(
        !out.starts_with("error:"),
        "ndjson error must be JSON: {out}"
    );
    assert!(!out.contains('\n'), "ndjson error must be one line: {out}");
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("output must be valid JSON");
    assert_eq!(parsed["error"]["exit_code"], err.exit_code());
    assert!(
        parsed.get("schema_version").is_none(),
        "ndjson error must stay bare (no schema_version): {out}"
    );
}

#[test]
fn format_dispatch_error_collision_carries_retry_tokens() {
    let err = BzrError::MidAirCollision {
        id: 12345,
        expected: "TOKEN-A".into(),
        actual: "2026-06-22T01:02:04Z".into(),
    };
    for format in [OutputFormat::Json, OutputFormat::Ndjson] {
        let out = format_dispatch_error(&err, format);
        let parsed: serde_json::Value =
            serde_json::from_str(&out).expect("output must be valid JSON");
        let inner = &parsed["error"];
        assert_eq!(inner["type"], "collision", "{out}");
        assert_eq!(inner["bug_id"], 12345, "{out}");
        assert_eq!(inner["last_change_time"], "2026-06-22T01:02:04Z", "{out}");
        assert_eq!(inner["if_match_token"], "TOKEN-A", "{out}");
    }
}

#[test]
fn format_dispatch_error_input_field_carries_field_and_value() {
    let err = BzrError::input_field(
        "deadline: 'nope' is not a valid date".into(),
        "deadline",
        Some("nope".into()),
    );
    let out = format_dispatch_error(&err, OutputFormat::Json);
    let parsed: serde_json::Value = serde_json::from_str(&out).expect("output must be valid JSON");
    assert_eq!(parsed["error"]["field"], "deadline", "{out}");
    assert_eq!(parsed["error"]["value"], "nope", "{out}");
}

#[test]
fn format_dispatch_error_universal_keys_present_and_correct_across_variants() {
    // Foreign-wrapped variants (Http(reqwest::Error), TomlParse) have no public
    // constructor and are covered transitively by the shared formatter path.
    let errors = [
        BzrError::Config("c".into()),
        BzrError::input("bad flag".into()),
        BzrError::input_field("m".into(), "f", Some("v".into())),
        BzrError::NotFound {
            resource: "bug",
            id: "1".into(),
        },
        BzrError::UnsupportedServerCapability {
            capability: "RedHat".into(),
            status: bzr::error::CAPABILITY_ABSENT,
            operation: "saved search 'triage'".into(),
            detail: "server does not implement it".into(),
        },
        BzrError::HttpStatus {
            status: 404,
            body: "nf".into(),
        },
        BzrError::Api {
            code: 101,
            message: "bad".into(),
        },
        BzrError::BatchPartialFailure {
            succeeded: 1,
            failed: 2,
        },
        BzrError::MidAirCollision {
            id: 9,
            expected: "a".into(),
            actual: "b".into(),
        },
    ];
    for err in &errors {
        let out = format_dispatch_error(err, OutputFormat::Ndjson);
        let parsed: serde_json::Value =
            serde_json::from_str(&out).expect("output must be valid JSON");
        let inner = &parsed["error"];
        assert_eq!(inner["type"], err.error_type(), "{out}");
        assert_eq!(inner["exit_code"], err.exit_code(), "{out}");
        assert_eq!(
            inner["message"].as_str().expect("message is string"),
            err.to_string(),
            "{out}"
        );
    }
}

async fn schema_from_command(name: &str) -> serde_json::Value {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let mut writers = bzr::output::writers::Writers::new(&mut out, &mut err);

    bzr::commands::schema::execute(
        Some(name),
        &bzr::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut writers,
    )
    .await
    .expect("schema must be published");

    serde_json::from_slice(&out).expect("schema output must be valid JSON")
}

fn assert_error_matches_schema(schema: &serde_json::Value, value: &serde_json::Value) {
    let envelope_required = schema
        .get("required")
        .and_then(serde_json::Value::as_array)
        .expect("schema must declare required top-level fields");
    assert!(
        envelope_required.iter().any(|field| field == "error"),
        "schema must require the top-level error envelope"
    );

    let error_schema = schema
        .pointer("/properties/error")
        .expect("schema must describe error property");
    let required = error_schema
        .get("required")
        .and_then(serde_json::Value::as_array)
        .expect("error property must declare required fields");
    let error = value
        .get("error")
        .and_then(serde_json::Value::as_object)
        .expect("formatted value must contain an error object");
    let properties = error_schema
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .expect("error property must declare nested properties");

    for key in ["type", "message", "exit_code"] {
        assert!(
            required.iter().any(|field| field == key),
            "schema must require error.{key}"
        );
        assert!(error.contains_key(key), "formatted error missing {key}");
        assert!(
            properties.contains_key(key),
            "schema must describe error.{key}"
        );
    }

    assert!(error["type"].is_string());
    assert!(error["message"].is_string());
    assert!(error["exit_code"].is_i64() || error["exit_code"].is_u64());

    let exit_code = error["exit_code"]
        .as_i64()
        .expect("formatted exit_code must fit in i64");
    let exit_code_schema = properties
        .get("exit_code")
        .expect("schema must describe error.exit_code");
    let minimum = exit_code_schema
        .get("minimum")
        .and_then(serde_json::Value::as_i64)
        .expect("schema must declare error.exit_code minimum");
    let maximum = exit_code_schema
        .get("maximum")
        .and_then(serde_json::Value::as_i64)
        .expect("schema must declare error.exit_code maximum");
    assert!(
        (minimum..=maximum).contains(&exit_code),
        "formatted exit_code {exit_code} outside schema bounds {minimum}..={maximum}"
    );

    // Enforce additionalProperties:false ourselves (the checks above do not):
    // every key the formatter emits must be declared in the schema, so a typo'd
    // or undeclared structured-detail key fails the test.
    for key in error.keys() {
        assert!(
            properties.contains_key(key),
            "emitted error.{key} is not declared in schemas/error.json error.properties"
        );
    }
}

#[tokio::test]
async fn format_dispatch_error_json_family_matches_published_schema() {
    let schema = schema_from_command("error").await;
    let errors = [
        BzrError::input("bad input".into()),
        BzrError::input_field("bad date".into(), "--deadline", Some("nope".into())),
        BzrError::NotFound {
            resource: "bug",
            id: "404".into(),
        },
        BzrError::UnsupportedServerCapability {
            capability: "RedHat".into(),
            status: bzr::error::CAPABILITY_ABSENT,
            operation: "saved search 'triage'".into(),
            detail: "server does not implement it".into(),
        },
        BzrError::HttpStatus {
            status: 404,
            body: "Not Found".into(),
        },
        BzrError::Api {
            code: 101,
            message: "Invalid Bug ID".into(),
        },
        BzrError::BatchPartialFailure {
            succeeded: 2,
            failed: 1,
        },
        BzrError::PinMismatch {
            server: "bugzilla.example".into(),
            expected: "AAAA".into(),
            actual: "BBBB".into(),
        },
        BzrError::MidAirCollision {
            id: 123,
            expected: "2026-06-22T01:02:03Z".into(),
            actual: "2026-06-22T01:02:04Z".into(),
        },
    ];

    for err in errors {
        for format in [OutputFormat::Json, OutputFormat::Ndjson] {
            let out = format_dispatch_error(&err, format);
            let parsed: serde_json::Value =
                serde_json::from_str(&out).expect("output must be valid JSON");
            assert_error_matches_schema(&schema, &parsed);
        }
    }
}

#[test]
fn format_dispatch_error_renders_table() {
    let err = BzrError::Config("bad config".into());
    let out = format_dispatch_error(&err, OutputFormat::Table);
    assert!(out.starts_with("error:"), "got {out:?}");
    assert!(out.contains("bad config"), "got {out:?}");
}

#[test]
fn format_dispatch_error_table_for_non_config_error() {
    let err = BzrError::Auth("kaboom".into());
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

/// `--json` / `--output ndjson` render the error message through the same
/// `Display` impl as the table path, so redaction lands on every format from
/// the one seam. Asserting all three here is what makes that shared, rather
/// than a property of the human path alone.
#[test]
fn format_dispatch_error_redacts_echoed_api_key_on_every_format() {
    let errs = [
        BzrError::Api {
            code: 32000,
            message: "bad request: /rest/bug?Bugzilla_api_key=SUPERSECRET&id=1".into(),
        },
        BzrError::HttpStatus {
            status: 401,
            body: "unauthorized for /rest/bug?Bugzilla_api_key=SUPERSECRET".into(),
        },
    ];
    for err in &errs {
        for format in [
            OutputFormat::Json,
            OutputFormat::Ndjson,
            OutputFormat::Table,
        ] {
            let out = format_dispatch_error(err, format);
            assert!(
                !out.contains("SUPERSECRET"),
                "{} leaked the key on {format:?}: {out}",
                err.error_type()
            );
            assert!(
                out.contains("Bugzilla_api_key=[REDACTED]"),
                "{} not redacted on {format:?}: {out}",
                err.error_type()
            );
        }
    }
}

#[cfg(feature = "test-helpers")]
#[tokio::test]
async fn format_dispatch_error_redacts_bare_configured_key_and_clears_context() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    let (mock, _tmp, config_path) = bzr::test_helpers::setup_isolated_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": true,
            "code": 32000,
            "message": "server rejected test-key"
        })))
        .mount(&mock)
        .await;

    for format in [
        OutputFormat::Json,
        OutputFormat::Ndjson,
        OutputFormat::Table,
    ] {
        let config = config_path.to_string_lossy();
        let cli = Cli::try_parse_from([
            "bzr",
            "--config",
            config.as_ref(),
            "--server",
            "test",
            "bug",
            "view",
            "42",
        ])
        .expect("CLI parses");
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut writers = bzr::output::writers::Writers::new(&mut stdout, &mut stderr);
        let err = bzr::dispatch(&cli, format, &mut writers)
            .await
            .expect_err("mocked Bugzilla error");

        let out = format_dispatch_error(&err, format);
        assert!(!out.contains("test-key"), "key leaked on {format:?}: {out}");
        assert!(out.contains("[REDACTED]"), "key not masked: {out}");

        let unrelated = BzrError::Api {
            code: 1,
            message: "unrelated test-key text".into(),
        };
        assert!(
            unrelated.to_string().contains("test-key"),
            "formatting did not clear the redaction context"
        );
    }
}
