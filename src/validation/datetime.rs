//! ISO-8601 date / datetime validation for Bugzilla search filters.
//!
//! Accepts four shapes (lengths fully constrain the format):
//!   * `YYYY-MM-DD`            (10 chars) — canonicalized to `YYYY-MM-DDT00:00:00Z`
//!   * `YYYY-MM-DDTHH:MM:SS`   (19 chars) — sent verbatim; server treats as UTC
//!   * `YYYY-MM-DDTHH:MM:SSZ`  (20 chars) — sent verbatim
//!   * `YYYY-MM-DDTHH:MM:SS±HH:MM` (25 chars) — sent verbatim
//!
//! Fractional seconds, week dates (`2026-W18-3`), and ordinal dates
//! (`2026-128`) are rejected: they are valid ISO-8601 but the server
//! is not guaranteed to accept them, and we'd rather fail fast at the
//! CLI than ship a malformed payload.

use crate::error::{BzrError, Result};

/// Validate an ISO-8601 datetime or bare date for use as a Bugzilla
/// search filter. On success, returns the canonical form sent to the
/// server (bare dates are expanded to `YYYY-MM-DDT00:00:00Z`).
///
/// `flag` is the CLI flag name (e.g. "--created-since") and is
/// included in the error message so callers can locate the offending
/// input when multiple date flags are in play.
pub fn parse_iso8601_or_date(s: &str, flag: &str) -> Result<String> {
    match try_canonicalize(s) {
        Some(canon) => Ok(canon),
        None => Err(BzrError::InputValidation(format!(
            "{flag}: '{s}' is not a valid ISO-8601 date or datetime.\n\
             Expected: YYYY-MM-DD, YYYY-MM-DDTHH:MM:SS, \
             YYYY-MM-DDTHH:MM:SSZ, or YYYY-MM-DDTHH:MM:SS±HH:MM"
        ))),
    }
}

/// Validate a bare `YYYY-MM-DD` date, returning it unchanged on success.
///
/// Unlike [`parse_iso8601_or_date`], this does **not** expand the value to a
/// datetime — it is for date-only fields such as the bug deadline, which
/// Bugzilla stores and echoes back as `YYYY-MM-DD`. `flag` is the CLI flag
/// name included in the error message.
pub fn parse_date_only(s: &str, flag: &str) -> Result<String> {
    if s.is_ascii() && s.len() == 10 && parse_date(s).is_some() {
        Ok(s.to_string())
    } else {
        Err(BzrError::InputValidation(format!(
            "{flag}: '{s}' is not a valid date. Expected: YYYY-MM-DD"
        )))
    }
}

/// Reduce a Bugzilla timestamp to a canonical comparison key (`YYYYMMDDHHMMSS`)
/// for the optimistic-concurrency guard (`bug update --expect-unchanged-since`).
///
/// Accepts both REST canonical forms (`2026-01-01T00:00:00Z`, the naive
/// `…T00:00:00`, and bare `2026-01-01`) and the XML-RPC basic form
/// (`20260101T00:00:00`), so the comparison is the same regardless of which
/// transport produced the `last_change_time`. Offset-bearing forms
/// (`…+01:00`) and anything else are rejected (`None`) — callers must compare
/// the server's UTC value, so two different instants are never keyed equal.
#[must_use]
pub fn timestamp_compare_key(s: &str) -> Option<String> {
    let trimmed = s.strip_suffix('Z').unwrap_or(s);
    let key: String = trimmed
        .chars()
        .filter(|c| !matches!(c, '-' | ':' | 'T'))
        .collect();
    if !key.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    match key.len() {
        14 => Some(key),
        8 => Some(format!("{key}000000")),
        _ => None,
    }
}

fn try_canonicalize(s: &str) -> Option<String> {
    // Length-check first; this also gates the byte-indexing below
    // (each branch knows the input is exactly that many bytes long).
    // ASCII-only because every accepted character is in [-+:0-9TZ].
    if !s.is_ascii() {
        return None;
    }
    match s.len() {
        10 => parse_date(s).map(|()| format!("{s}T00:00:00Z")),
        19 => parse_datetime_naive(s).map(|()| s.to_string()),
        20 => parse_datetime_z(s).map(|()| s.to_string()),
        25 => parse_datetime_offset(s).map(|()| s.to_string()),
        _ => None,
    }
}

fn parse_date(s: &str) -> Option<()> {
    let bytes = s.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    if !bytes[..4].iter().all(u8::is_ascii_digit)
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || !bytes[8..10].iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    let month: u32 = s[5..7].parse().ok()?;
    let day: u32 = s[8..10].parse().ok()?;
    let year: u32 = s[..4].parse().ok()?;
    let max_day = days_in_month(year, month)?;
    if day == 0 || day > max_day {
        return None;
    }
    Some(())
}

fn days_in_month(year: u32, month: u32) -> Option<u32> {
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return None,
    };
    Some(days)
}

fn is_leap_year(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

fn parse_time(s: &str) -> Option<()> {
    debug_assert_eq!(s.len(), 8, "parse_time requires exactly 8 bytes (HH:MM:SS)");
    let bytes = s.as_bytes();
    if bytes[2] != b':' || bytes[5] != b':' {
        return None;
    }
    if !bytes[..2].iter().all(u8::is_ascii_digit)
        || !bytes[3..5].iter().all(u8::is_ascii_digit)
        || !bytes[6..8].iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    let h: u32 = s[..2].parse().ok()?;
    let m: u32 = s[3..5].parse().ok()?;
    let sec: u32 = s[6..8].parse().ok()?;
    // Allow second == 60 for leap seconds.
    if h > 23 || m > 59 || sec > 60 {
        return None;
    }
    Some(())
}

fn parse_datetime_naive(s: &str) -> Option<()> {
    if s.as_bytes()[10] != b'T' {
        return None;
    }
    parse_date(&s[..10])?;
    parse_time(&s[11..19])?;
    Some(())
}

fn parse_datetime_z(s: &str) -> Option<()> {
    if !s.ends_with('Z') {
        return None;
    }
    parse_datetime_naive(&s[..19])?;
    Some(())
}

fn parse_datetime_offset(s: &str) -> Option<()> {
    parse_datetime_naive(&s[..19])?;
    let bytes = s.as_bytes();
    let sign = bytes[19];
    if sign != b'+' && sign != b'-' {
        return None;
    }
    if bytes[22] != b':' {
        return None;
    }
    if !bytes[20..22].iter().all(u8::is_ascii_digit)
        || !bytes[23..25].iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    let h: u32 = s[20..22].parse().ok()?;
    let m: u32 = s[23..25].parse().ok()?;
    if h > 14 || m > 59 || (h == 14 && m > 0) {
        return None;
    }
    Some(())
}

#[cfg(test)]
#[path = "datetime_tests.rs"]
mod tests;
