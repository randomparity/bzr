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
