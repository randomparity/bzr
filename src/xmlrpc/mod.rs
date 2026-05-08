//! XML-RPC transport adapter for Bugzilla servers that use the XML-RPC API
//! instead of (or alongside) the REST API. Used internally by `BugzillaClient`
//! when the detected `ApiMode` is `XmlRpc` or `Hybrid`.

mod call;
pub(crate) mod client;
mod fault;
mod parsing;
pub(crate) mod value;

pub use call::build_request;
pub use parsing::parse_response;
pub use value::Value;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
