#![expect(clippy::disallowed_methods, clippy::unwrap_used, clippy::panic)]

use super::*;

// ── Timeout + retry helpers (#311) ──────────────────────────────────

#[test]
fn is_retryable_status_covers_429_and_5xx_only() {
    assert!(is_retryable_status(429));
    assert!(is_retryable_status(500));
    assert!(is_retryable_status(503));
    assert!(is_retryable_status(599));
    assert!(!is_retryable_status(200));
    assert!(!is_retryable_status(400));
    assert!(!is_retryable_status(401));
    assert!(!is_retryable_status(404));
    assert!(!is_retryable_status(418));
}

#[test]
fn should_retry_status_gates_5xx_on_safety() {
    // 429 is always retryable (rate-limited, not processed).
    assert!(should_retry_status(429, false));
    assert!(should_retry_status(429, true));
    // 5xx only for safe (GET/HEAD) requests; a write 500 may have been applied.
    assert!(should_retry_status(503, true));
    assert!(!should_retry_status(503, false));
    assert!(should_retry_status(500, true));
    assert!(!should_retry_status(500, false));
    // Non-transient statuses are never retried regardless of safety.
    assert!(!should_retry_status(404, true));
    assert!(!should_retry_status(200, true));
}

#[test]
fn backoff_delay_grows_exponentially_and_caps() {
    let d0 = backoff_delay(0, None);
    let d1 = backoff_delay(1, None);
    let d2 = backoff_delay(2, None);
    assert!(d1 > d0, "attempt 1 should wait longer than attempt 0");
    assert!(d2 > d1, "attempt 2 should wait longer than attempt 1");
    // Far-out attempts are capped, never unbounded.
    assert!(backoff_delay(20, None) <= std::time::Duration::from_secs(30));
}

#[test]
fn backoff_delay_honors_retry_after_when_longer() {
    // A long server Retry-After overrides the short exponential base.
    let ra = std::time::Duration::from_secs(10);
    assert_eq!(backoff_delay(0, Some(ra)), ra);
    // But a Retry-After is still capped.
    let huge = std::time::Duration::from_secs(9999);
    assert!(backoff_delay(0, Some(huge)) <= std::time::Duration::from_secs(30));
    // A tiny Retry-After never shortens the exponential base.
    let tiny = std::time::Duration::from_millis(1);
    assert!(backoff_delay(3, Some(tiny)) >= backoff_delay(3, None));
}

#[test]
fn parse_retry_after_reads_delta_seconds() {
    assert_eq!(
        parse_retry_after("5"),
        Some(std::time::Duration::from_secs(5))
    );
    assert_eq!(
        parse_retry_after("  12 "),
        Some(std::time::Duration::from_secs(12))
    );
}

#[test]
fn parse_retry_after_rejects_non_integer() {
    // HTTP-date form is unsupported (no date dep); falls back to backoff.
    assert_eq!(parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT"), None);
    assert_eq!(parse_retry_after("soon"), None);
    assert_eq!(parse_retry_after(""), None);
}

#[test]
fn resolve_timeout_secs_prefers_flag_then_env() {
    assert_eq!(resolve_timeout_secs(Some(15), Some("99")), Some(15));
    assert_eq!(resolve_timeout_secs(None, Some("99")), Some(99));
    assert_eq!(resolve_timeout_secs(None, None), None);
}

#[test]
fn resolve_timeout_secs_ignores_invalid_env() {
    assert_eq!(resolve_timeout_secs(None, Some("0")), None);
    assert_eq!(resolve_timeout_secs(None, Some("-3")), None);
    assert_eq!(resolve_timeout_secs(None, Some("abc")), None);
}

#[test]
fn utf8_prefix_backs_off_without_exceeding_max() {
    // "é" is two bytes (0xC3 0xA9). A 2-byte cap lands mid-'é', so the prefix
    // must shrink to "h" (1 byte) — a guard that grew `end` instead of
    // shrinking it would return "hé" (3 bytes), exceeding the cap.
    let s = "héllo";
    let p = utf8_prefix(s, 2);
    assert_eq!(p, "h");
    assert!(p.len() <= 2, "prefix must never exceed max_bytes");
    // A cap on an ASCII boundary returns exactly that many bytes.
    assert_eq!(utf8_prefix("abcdef", 3), "abc");
    // A cap at or beyond the length returns the whole string.
    assert_eq!(utf8_prefix(s, 100), s);
}

// ── Bounded response-body reads (#740) ──────────────────────────────

/// Serve `body` once over wiremock and read it back under `limit`.
async fn read_served_body(
    body: &str,
    limit: u64,
) -> std::result::Result<String, super::BodyReadError> {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;
    let response = reqwest::Client::new()
        .get(server.uri())
        .send()
        .await
        .unwrap();
    read_body_within(response, "test read", limit).await
}

#[tokio::test]
async fn read_body_within_refuses_a_body_over_the_limit() {
    let error = read_served_body(&"a".repeat(17), 16).await.unwrap_err();

    let BodyReadError::TooLarge {
        operation,
        limit_bytes,
    } = error
    else {
        panic!("expected TooLarge, got {error:?}");
    };
    assert_eq!(operation, "test read");
    assert_eq!(limit_bytes, 16);
}

#[tokio::test]
async fn read_body_within_returns_a_body_at_the_limit() {
    let body = "a".repeat(16);
    assert_eq!(read_served_body(&body, 16).await.unwrap(), body);
}

#[tokio::test]
async fn read_body_within_returns_a_short_body() {
    assert_eq!(read_served_body("hi", 16).await.unwrap(), "hi");
}

#[tokio::test]
async fn read_body_within_reports_a_transport_failure() {
    // wiremock cannot express a truncated body, so serve one from a raw socket:
    // declare 32 bytes, write four, then drop the connection mid-body.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        let Ok((mut stream, _addr)) = listener.accept() else {
            return;
        };
        let _ = std::io::Read::read(&mut stream, &mut [0_u8; 1024]);
        let _ = std::io::Write::write_all(
            &mut stream,
            b"HTTP/1.1 200 OK\r\nContent-Length: 32\r\n\r\noops",
        );
    });

    let response = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}"))
        .send()
        .await
        .unwrap();
    let error = read_body_within(response, "test read", 1024)
        .await
        .unwrap_err();
    handle.join().unwrap();

    assert!(
        matches!(error, BodyReadError::Transport(_)),
        "expected Transport, got {error:?}",
    );
}

#[tokio::test]
async fn read_body_bounded_returns_a_normal_body() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("{\"ok\":true}"))
        .mount(&server)
        .await;
    let response = reqwest::Client::new()
        .get(server.uri())
        .send()
        .await
        .unwrap();

    let body = read_body_bounded(response, "test read").await.unwrap();

    assert_eq!(body, "{\"ok\":true}");
}

#[test]
fn max_response_body_bytes_is_64_mib() {
    assert_eq!(MAX_RESPONSE_BODY_BYTES, 64 * 1024 * 1024);
}

#[test]
fn body_read_error_maps_too_large_to_response_too_large() {
    let error = crate::error::BzrError::from(BodyReadError::TooLarge {
        operation: "response body".to_owned(),
        limit_bytes: MAX_RESPONSE_BODY_BYTES,
    });

    assert_eq!(error.exit_code(), 16);
    assert_eq!(error.error_type(), "response_too_large");
}

/// Every unbounded `reqwest::Response` body reader. `.json(` is deliberately
/// absent: `RequestBuilder::json` shares the name and is a request body, which
/// this bound does not cover. `.bytes()` is matched only with its `.await`,
/// because `str::bytes` is common and synchronous.
const UNBOUNDED_BODY_READERS: &[&str] = &[
    ".text().await",
    ".text_with_charset(",
    ".bytes().await",
    ".bytes_stream()",
    ".json().await",
    ".json::<",
    // `read_body_within` is built on `chunk()`; anywhere else it is a hand-rolled
    // accumulation that reimplements the bound without it.
    ".chunk().await",
];

/// The one file allowed to read a response body without the helper, because it
/// *is* the helper. Matched by its path under `src/`, not by base name: a
/// future `src/client/http.rs` must not inherit the exemption.
const BOUND_IMPLEMENTATION: &str = "http.rs";

/// The bound exists at thirteen call sites with no single chokepoint to place
/// it at, so nothing but this test stops a fourteenth unbounded read appearing.
///
/// Whitespace is collapsed before matching, so a rustfmt-wrapped `.text()`
/// and `.await` on separate lines is still caught; that costs the line number,
/// so offenders are reported by file.
#[test]
fn no_unbounded_response_body_reads_outside_tests() {
    fn walk(dir: &std::path::Path, root: &std::path::Path, offenders: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, root, offenders);
                continue;
            }
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.extension() != Some(std::ffi::OsStr::new("rs"))
                || name.ends_with("_tests.rs")
                || path == root.join(BOUND_IMPLEMENTATION)
            {
                continue;
            }
            let source: String = std::fs::read_to_string(&path)
                .unwrap()
                .split_whitespace()
                .collect();
            for reader in UNBOUNDED_BODY_READERS {
                if source.contains(reader) {
                    offenders.push(format!("{} ({reader})", path.display()));
                }
            }
        }
    }

    let mut offenders = Vec::new();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    walk(&root, &root, &mut offenders);

    assert!(
        offenders.is_empty(),
        "response bodies must be read through crate::http::read_body_bounded, \
         which bounds them; these read unbounded: {offenders:?}",
    );
}
