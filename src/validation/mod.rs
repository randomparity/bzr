//! Shared client-side input validators.
//!
//! Validators here are not tied to a single command — they live in this
//! module so that any subcommand parsing the same value-shape can call
//! a single canonical implementation. On failure, validators return
//! `BzrError::InputValidation`, which exits with code 7.

pub mod datetime;

pub use datetime::{parse_date_only, parse_iso8601_or_date};

use crate::error::Result;

/// Validate an optional date string for use as a Bugzilla search filter.
///
/// `None` is passed through unchanged; `Some(s)` is canonicalized via
/// [`parse_iso8601_or_date`]. Wraps the common
/// `opt.as_deref().map(|s| parse_iso8601_or_date(s, flag)).transpose()`
/// idiom used at every CLI date-flag site.
pub fn parse_optional_date(opt: Option<&str>, flag: &str) -> Result<Option<String>> {
    opt.map(|s| parse_iso8601_or_date(s, flag)).transpose()
}

/// Validate an optional bare `YYYY-MM-DD` date for a date-only field.
///
/// `None` is passed through unchanged; `Some(s)` is validated via
/// [`parse_date_only`] and returned verbatim (no datetime expansion).
pub fn parse_optional_date_only(opt: Option<&str>, flag: &str) -> Result<Option<String>> {
    opt.map(|s| parse_date_only(s, flag)).transpose()
}
