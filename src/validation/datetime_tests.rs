#![expect(clippy::unwrap_used)]

use super::parse_iso8601_or_date;

const FLAG: &str = "--created-since";

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
}
