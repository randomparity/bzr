//! Cross-cutting command infrastructure shared across resource handlers.
//!
//! Unlike the per-resource modules at the `commands` top level, these modules
//! provide the runtime plumbing every command relies on. `invocation` owns
//! per-command state, capability policy, and inline server configuration;
//! `input` owns payload loading, Bugzilla URL import, attachment input, and flag
//! syntax parsing; `interaction` owns prompts and `$EDITOR`; `search` owns
//! query execution, field projection, and paging; `shared` owns connection/body
//! helpers; and `mutation` owns the admin create/update driver.

pub mod input;
pub mod interaction;
pub mod invocation;
pub(crate) mod mutation;
pub(crate) mod search;
pub(crate) mod shared;
