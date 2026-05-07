//! Shared client-side input validators.
//!
//! Validators here are not tied to a single command — they live in this
//! module so that any subcommand parsing the same value-shape can call
//! a single canonical implementation. On failure, validators return
//! `BzrError::InputValidation`, which exits with code 7.

pub mod datetime;

pub use datetime::parse_iso8601_or_date;
