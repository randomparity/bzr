//! XML-RPC transport adapter for Bugzilla servers that use the XML-RPC API
//! instead of (or alongside) the REST API. Used internally by `BugzillaClient`
//! when the detected `ApiMode` is `XmlRpc` or `Hybrid`.

pub(crate) mod call;
pub(crate) mod client;
mod fault;
pub(crate) mod parsing;
pub(crate) mod value;

#[cfg(fuzzing)]
#[doc(hidden)]
pub use parsing::parse_response;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
