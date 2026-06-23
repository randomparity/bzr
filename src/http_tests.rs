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
