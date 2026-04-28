//! XML-RPC fault response parsing.

use crate::error::BzrError;
use crate::xmlrpc::Value;

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
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn fault_struct_maps_to_api_error() {
        let mut members = BTreeMap::new();
        members.insert("faultCode".into(), Value::Int(102));
        members.insert("faultString".into(), Value::String("Access denied".into()));
        let err = fault_to_error(&Value::Struct(members));
        assert!(
            matches!(&err, BzrError::Api { code, message } if *code == 102 && message == "Access denied"),
            "expected Api error with code 102 and matching message, got {err:?}"
        );
    }

    #[test]
    fn fault_missing_fields_uses_defaults() {
        let err = fault_to_error(&Value::Struct(BTreeMap::new()));
        assert!(
            matches!(&err, BzrError::Api { code, message } if *code == -1 && message == "unknown fault"),
            "expected Api error with default code/message, got {err:?}"
        );
    }

    #[test]
    fn non_struct_fault_is_xmlrpc_error() {
        let err = fault_to_error(&Value::String("oops".into()));
        assert!(matches!(err, BzrError::XmlRpc(_)));
    }
}
