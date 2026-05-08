#![expect(clippy::unwrap_used)]

use std::collections::BTreeMap;

use super::*;

#[test]
fn value_conversions() {
    assert_eq!(Value::from("hello").as_str().unwrap(), "hello");
    assert_eq!(Value::from(42i64).as_i64().unwrap(), 42);
    assert!(Value::from(true).as_bool().unwrap());

    let s = Value::String("test".into());
    assert!(s.as_i64().is_none());
    assert!(s.as_bool().is_none());
    assert!(s.as_struct().is_none());
    assert!(s.as_array().is_none());
    assert!(s.as_f64().is_none());
}

#[test]
fn from_string_value() {
    let v: Value = "owned".to_string().into();
    assert_eq!(v.as_str().unwrap(), "owned");
}

#[test]
fn from_vec_and_btreemap_value() {
    let arr: Value = vec![Value::Int(1), Value::Int(2)].into();
    assert_eq!(arr.as_array().unwrap().len(), 2);

    let mut m = BTreeMap::new();
    m.insert("k".into(), Value::from("v"));
    let s: Value = m.into();
    assert_eq!(s.as_struct().unwrap().get("k").unwrap().as_str(), Some("v"));
}
