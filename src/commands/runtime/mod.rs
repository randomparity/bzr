//! Cross-cutting command infrastructure shared across resource handlers.
//!
//! Unlike the per-resource modules at the `commands` top level, these modules
//! provide the runtime plumbing every command relies on: explicit invocation
//! context (`context`), connection/body/merge helpers (`shared`), result
//! paging (`paging`), Bugzilla flag-syntax parsing ([`flags`]), the `$EDITOR`
//! launcher (`editor`), the admin create/update driver (`mutation`), and
//! inline server data types ([`inline_server`]).

pub mod confirm;
pub mod context;
pub(crate) mod editor;
pub mod flags;
pub(crate) mod from_json;
pub mod inline_server;
pub(crate) mod mutation;
pub(crate) mod paging;
pub(crate) mod search;
pub(crate) mod shared;
pub(crate) mod url_parser;
