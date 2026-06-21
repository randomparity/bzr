//! Cross-cutting command infrastructure shared across resource handlers.
//!
//! Unlike the per-resource modules at the `commands` top level, these modules
//! provide the runtime plumbing every command relies on: process-global flags
//! ([`confirm`], [`dry_run`]), connection setup ([`shared`]), result paging
//! ([`paging`]), Bugzilla flag-syntax parsing ([`flags`]), the `$EDITOR`
//! launcher ([`editor`]), inline server overrides ([`inline_server`]), and the
//! body-source helpers used to read input from args/files/stdin ([`shared`]).

pub mod confirm;
pub mod dry_run;
pub(crate) mod editor;
pub mod flags;
pub(crate) mod from_json;
pub mod inline_server;
pub(crate) mod paging;
pub(crate) mod shared;
