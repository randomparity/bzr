//! Shared client-side input validators.
//!
//! Validators here are not tied to a single command — they live in this
//! module so that any subcommand parsing the same value-shape can call
//! a single canonical implementation. On failure, validators return
//! `BzrError::InputValidation`, which exits with code 7.

pub mod datetime;

pub use datetime::{parse_date_only, parse_iso8601_or_date, timestamp_compare_key};

use crate::error::Result;
use crate::field_aliases::resolve_field_alias;
use crate::types::common::SortDirection;

/// Validate an optional search date flag.
pub fn parse_optional_date(opt: Option<&str>, flag: &str) -> Result<Option<String>> {
    opt.map(|s| parse_iso8601_or_date(s, flag)).transpose()
}

/// Validate an optional date-only flag.
pub fn parse_optional_date_only(opt: Option<&str>, flag: &str) -> Result<Option<String>> {
    opt.map(|s| parse_date_only(s, flag)).transpose()
}

/// Build the Bugzilla `order` clause from `--sort`/`--order`.
///
/// With a sort field, returns `<resolved-field> <DIR>` plus a `bug_id`
/// tiebreaker (omitted when the field already is `bug_id`) so ties resolve
/// deterministically. Without a sort field, returns the stable default
/// `bug_id`, so identical runs return rows in the same order even with no
/// `--sort`. The field name is normalized through the same alias table as
/// the filter flags (e.g. `assignee` -> `assigned_to`).
#[must_use]
pub fn build_order(sort: Option<&str>, direction: SortDirection) -> String {
    match sort {
        Some(field) => {
            let resolved = resolve_field_alias(field);
            if resolved == "bug_id" {
                format!("bug_id {}", direction.keyword())
            } else {
                format!("{} {}, bug_id", resolved, direction.keyword())
            }
        }
        None => "bug_id".to_string(),
    }
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
