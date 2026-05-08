//! Output formatting modules.
//!
//! Command modules import resource writers from their owning leaf module so
//! unused facade exports do not accumulate as output formats change.

mod formatting;
pub(crate) mod resources;
pub(crate) mod result_types;
pub mod writers;
