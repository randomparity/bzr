//! XML-RPC fault response parsing.

use crate::error::BzrError;
use crate::xmlrpc::value::Value;

/// Convert an XML-RPC `<fault>` value to a `BzrError`.
///
/// Faults are mapped to `BzrError::Api` for consistent formatting with REST
/// API errors. A fault that is not a struct is reported as a malformed
/// response via `BzrError::XmlRpc`.
pub(super) fn fault_to_error(value: &Value) -> BzrError {
    if let Some(members) = value.as_struct() {
        let code = members
            .get("faultCode")
            .and_then(Value::as_i64)
            .unwrap_or(-1);
        let msg = members
            .get("faultString")
            .and_then(Value::as_str)
            .unwrap_or("unknown fault")
            .to_string();
        BzrError::Api { code, message: msg }
    } else {
        BzrError::XmlRpc("malformed fault response".into())
    }
}

#[cfg(test)]
#[path = "fault_tests.rs"]
mod tests;
