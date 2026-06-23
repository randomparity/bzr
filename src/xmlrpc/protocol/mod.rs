//! XML-RPC wire protocol support: request building, response parsing, fault
//! conversion, value representation, and the HTTP transport wrapper.

pub(crate) mod call;
pub(crate) mod client;
pub(crate) mod fault;
pub(crate) mod parsing;
pub(crate) mod value;

pub(crate) use client::XmlRpcClient;
pub(crate) use value::Value;

#[cfg(fuzzing)]
#[doc(hidden)]
pub use parsing::parse_response;
