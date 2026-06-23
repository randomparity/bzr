//! XML-RPC transport adapter for Bugzilla servers that use the XML-RPC API
//! instead of (or alongside) the REST API. Used internally by `BugzillaClient`
//! when the detected `ApiMode` is `XmlRpc` or `Hybrid`.

pub(crate) mod protocol;
mod resources;

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
