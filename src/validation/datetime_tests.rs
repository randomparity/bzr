#![expect(clippy::unwrap_used)]

use super::{parse_date_only, parse_iso8601_or_date, timestamp_compare_key};

const FLAG: &str = "--created-since";

#[test]
fn timestamp_compare_key_is_transport_agnostic() {
    // REST canonical (Z), REST naive, and XML-RPC basic forms of the same
    // instant all key identically.
    let rest_z = timestamp_compare_key("2026-06-19T12:00:00Z").unwrap();
    let rest_naive = timestamp_compare_key("2026-06-19T12:00:00").unwrap();
    let xmlrpc_basic = timestamp_compare_key("20260619T12:00:00").unwrap();
    assert_eq!(rest_z, "20260619120000");
    assert_eq!(rest_z, rest_naive);
    assert_eq!(rest_z, xmlrpc_basic);
    // A bare date expands to midnight.
    assert_eq!(
        timestamp_compare_key("2026-06-19").unwrap(),
        "20260619000000"
    );
}

#[test]
fn timestamp_compare_key_rejects_offsets_and_garbage() {
    // Offset forms are rejected so two different instants are never keyed equal.
    assert!(timestamp_compare_key("2026-06-19T12:00:00+01:00").is_none());
    assert!(timestamp_compare_key("not-a-timestamp").is_none());
    assert!(timestamp_compare_key("").is_none());
    // Different instants key differently.
    assert_ne!(
        timestamp_compare_key("2026-06-19T12:00:00Z"),
        timestamp_compare_key("2026-06-19T13:00:00Z")
    );
}

#[test]
fn accepts_bare_date_and_canonicalizes() {
    let canon = parse_iso8601_or_date("2026-04-01", FLAG).unwrap();
    assert_eq!(canon, "2026-04-01T00:00:00Z");
}

#[test]
fn accepts_naive_datetime_unchanged() {
    let canon = parse_iso8601_or_date("2026-04-01T12:30:45", FLAG).unwrap();
    assert_eq!(canon, "2026-04-01T12:30:45");
}

#[test]
fn accepts_zulu_datetime_unchanged() {
    let canon = parse_iso8601_or_date("2026-04-01T12:30:45Z", FLAG).unwrap();
    assert_eq!(canon, "2026-04-01T12:30:45Z");
}

#[test]
fn accepts_positive_offset_unchanged() {
    let canon = parse_iso8601_or_date("2026-04-01T12:30:45+05:30", FLAG).unwrap();
    assert_eq!(canon, "2026-04-01T12:30:45+05:30");
}

#[test]
fn accepts_negative_offset_unchanged() {
    let canon = parse_iso8601_or_date("2026-04-01T12:30:45-08:00", FLAG).unwrap();
    assert_eq!(canon, "2026-04-01T12:30:45-08:00");
}

#[test]
fn accepts_leap_second() {
    // 2016-12-31T23:59:60Z was an actual leap second.
    let canon = parse_iso8601_or_date("2016-12-31T23:59:60Z", FLAG).unwrap();
    assert_eq!(canon, "2016-12-31T23:59:60Z");
}

#[test]
fn rejects_empty_string() {
    let err = parse_iso8601_or_date("", FLAG).unwrap_err();
    assert!(err.to_string().contains(FLAG));
}

#[test]
fn rejects_garbage_word() {
    let err = parse_iso8601_or_date("tomorrow", FLAG).unwrap_err();
    assert!(err.to_string().contains("tomorrow"));
    assert!(err.to_string().contains(FLAG));
}

#[test]
fn rejects_year_only() {
    assert!(parse_iso8601_or_date("2026", FLAG).is_err());
}

#[test]
fn rejects_year_month() {
    assert!(parse_iso8601_or_date("2026-04", FLAG).is_err());
}

#[test]
fn rejects_slash_separators() {
    assert!(parse_iso8601_or_date("2026/04/01", FLAG).is_err());
}

#[test]
fn rejects_space_separator_between_date_and_time() {
    // Same length as the T form but uses space — strict T required.
    assert!(parse_iso8601_or_date("2026-04-01 12:30:45", FLAG).is_err());
}

#[test]
fn rejects_fractional_seconds() {
    assert!(parse_iso8601_or_date("2026-04-01T12:30:45.123Z", FLAG).is_err());
}

#[test]
fn rejects_week_date() {
    assert!(parse_iso8601_or_date("2026-W18-3", FLAG).is_err());
}

#[test]
fn rejects_ordinal_date() {
    assert!(parse_iso8601_or_date("2026-128", FLAG).is_err());
}

#[test]
fn rejects_zero_month() {
    assert!(parse_iso8601_or_date("2026-00-15", FLAG).is_err());
}

#[test]
fn rejects_month_above_twelve() {
    assert!(parse_iso8601_or_date("2026-13-01", FLAG).is_err());
}

#[test]
fn rejects_zero_day() {
    assert!(parse_iso8601_or_date("2026-04-00", FLAG).is_err());
}

#[test]
fn rejects_day_above_thirty_one() {
    assert!(parse_iso8601_or_date("2026-04-32", FLAG).is_err());
}

#[test]
fn rejects_day_past_month_length() {
    assert!(parse_iso8601_or_date("2026-04-31", FLAG).is_err());
    assert!(parse_iso8601_or_date("2026-02-30", FLAG).is_err());
}

#[test]
fn rejects_february_twenty_ninth_on_non_leap_year() {
    assert!(parse_iso8601_or_date("2025-02-29", FLAG).is_err());
}

#[test]
fn accepts_february_twenty_ninth_on_leap_year() {
    let canon = parse_iso8601_or_date("2024-02-29", FLAG).unwrap();
    assert_eq!(canon, "2024-02-29T00:00:00Z");
}

#[test]
fn rejects_february_twenty_ninth_on_century_non_leap_year() {
    // 1900 is divisible by 100 but not 400, so it is NOT a leap year.
    assert!(parse_iso8601_or_date("1900-02-29", FLAG).is_err());
}

#[test]
fn accepts_february_twenty_ninth_on_quadricentennial_leap_year() {
    // 2000 is divisible by 400, so it IS a leap year.
    let canon = parse_iso8601_or_date("2000-02-29", FLAG).unwrap();
    assert_eq!(canon, "2000-02-29T00:00:00Z");
}

#[test]
fn accepts_year_zero() {
    // Year 0000 is divisible by 400 (leap). The validator is purely
    // structural — it does not impose a lower year bound — so 0000-01-01
    // is accepted and canonicalized. Bugzilla itself rejects such dates;
    // we fail fast only on malformed shapes, not implausible-but-valid ones.
    let canon = parse_iso8601_or_date("0000-01-01", FLAG).unwrap();
    assert_eq!(canon, "0000-01-01T00:00:00Z");
}

#[test]
fn rejects_hour_above_twenty_three() {
    assert!(parse_iso8601_or_date("2026-04-01T25:00:00Z", FLAG).is_err());
}

#[test]
fn rejects_minute_above_fifty_nine() {
    assert!(parse_iso8601_or_date("2026-04-01T12:60:00Z", FLAG).is_err());
}

#[test]
fn rejects_second_above_sixty() {
    assert!(parse_iso8601_or_date("2026-04-01T12:30:61Z", FLAG).is_err());
}

#[test]
fn rejects_offset_without_colon() {
    assert!(parse_iso8601_or_date("2026-04-01T12:30:45+0530", FLAG).is_err());
}

#[test]
fn rejects_offset_with_invalid_sign() {
    assert!(parse_iso8601_or_date("2026-04-01T12:30:45 05:30", FLAG).is_err());
}

#[test]
fn rejects_offset_hours_above_fourteen() {
    assert!(parse_iso8601_or_date("2026-04-01T12:30:45+15:00", FLAG).is_err());
}

#[test]
fn rejects_offset_minutes_when_hour_is_fourteen() {
    assert!(parse_iso8601_or_date("2026-04-01T12:30:45+14:01", FLAG).is_err());
}

#[test]
fn rejects_offset_minutes_above_fifty_nine() {
    assert!(parse_iso8601_or_date("2026-04-01T12:30:45+05:60", FLAG).is_err());
}

#[test]
fn rejects_non_ascii_input() {
    // 10 characters but non-ASCII — must not panic on byte indexing.
    assert!(parse_iso8601_or_date("2026-04-0\u{00E9}", FLAG).is_err());
}

#[test]
fn error_message_includes_expected_forms() {
    let err = parse_iso8601_or_date("nope", FLAG).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("YYYY-MM-DD"));
    assert!(msg.contains("YYYY-MM-DDTHH:MM:SS"));
    assert!(msg.contains("YYYY-MM-DDTHH:MM:SSZ"));
    assert!(msg.contains("YYYY-MM-DDTHH:MM:SS±HH:MM"));
}

#[test]
fn date_only_accepts_bare_date_verbatim() {
    // Unlike parse_iso8601_or_date, no datetime expansion.
    assert_eq!(
        parse_date_only("2026-12-31", "--deadline").unwrap(),
        "2026-12-31"
    );
}

#[test]
fn date_only_rejects_datetime() {
    assert!(parse_date_only("2026-12-31T00:00:00Z", "--deadline").is_err());
}

#[test]
fn date_only_rejects_garbage_and_names_flag() {
    let err = parse_date_only("garbage", "--deadline").unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(err.to_string().contains("--deadline"));
}

#[test]
fn date_only_rejects_century_non_leap_feb_29() {
    assert!(parse_date_only("1900-02-29", "--deadline").is_err());
}
