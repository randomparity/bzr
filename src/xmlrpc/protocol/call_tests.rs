use std::collections::BTreeMap;

use crate::xmlrpc::protocol::Value;

use super::build_request;

#[test]
fn build_simple_request() {
    let mut params = BTreeMap::new();
    params.insert("product".into(), Value::from("TestProduct"));
    params.insert("limit".into(), Value::Int(10));

    let xml = build_request("Bug.search", params);

    assert!(xml.contains("<methodName>Bug.search</methodName>"));
    assert!(xml.contains("<name>limit</name><value><int>10</int></value>"));
    assert!(xml.contains("<name>product</name><value><string>TestProduct</string></value>"));
}

#[test]
fn build_request_with_bool_and_array() {
    let mut params = BTreeMap::new();
    params.insert("active".into(), Value::Bool(true));
    params.insert(
        "ids".into(),
        Value::Array(vec![Value::Int(1), Value::Int(2)]),
    );

    let xml = build_request("Bug.get", params);

    assert!(xml.contains("<boolean>1</boolean>"));
    assert!(xml.contains(
        "<array><data><value><int>1</int></value><value><int>2</int></value></data></array>"
    ));
}

#[test]
fn build_request_escapes_special_chars() {
    let mut params = BTreeMap::new();
    params.insert("query".into(), Value::from("foo & bar <baz>"));

    let xml = build_request("Test.method", params);

    assert!(xml.contains("foo &amp; bar &lt;baz&gt;"));
}

#[test]
fn roundtrip_nested_struct() {
    let mut inner = BTreeMap::new();
    inner.insert("key".into(), Value::from("val"));
    let mut params = BTreeMap::new();
    params.insert("nested".into(), Value::Struct(inner));

    let xml = build_request("Test", params);
    assert!(xml.contains("<name>nested</name>"));
    assert!(xml.contains("<name>key</name><value><string>val</string></value>"));
}

#[test]
fn build_request_renders_double_and_datetime_and_base64() {
    let mut params = BTreeMap::new();
    params.insert("score".into(), Value::Double(1.5));
    params.insert("when".into(), Value::DateTime("20250101T00:00:00".into()));
    params.insert("payload".into(), Value::Base64(b"Hello".to_vec()));

    let xml = build_request("Test", params);

    assert!(xml.contains("<double>1.5</double>"));
    assert!(xml.contains("<dateTime.iso8601>20250101T00:00:00</dateTime.iso8601>"));
    assert!(xml.contains("<base64>SGVsbG8=</base64>"));
}
