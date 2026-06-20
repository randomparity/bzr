use std::collections::BTreeMap;

use super::{get_datetime_str, get_int_array, get_nonempty_str, get_str_array};
use crate::xmlrpc::value::Value;

#[test]
fn get_nonempty_str_filters_empty_and_non_string() {
    let mut m = BTreeMap::new();
    m.insert("empty".into(), Value::String(String::new()));
    m.insert("filled".into(), Value::String("x".into()));
    m.insert("not_string".into(), Value::Int(5));
    assert!(get_nonempty_str(&m, "empty").is_none());
    assert_eq!(get_nonempty_str(&m, "filled").as_deref(), Some("x"));
    assert!(get_nonempty_str(&m, "not_string").is_none());
    assert!(get_nonempty_str(&m, "missing").is_none());
}

#[test]
fn get_datetime_str_covers_datetime_string_and_fallthrough() {
    let mut m = BTreeMap::new();
    m.insert("dt".into(), Value::DateTime("2024-01-01T00:00:00".into()));
    m.insert("s_full".into(), Value::String("2024-02-02".into()));
    m.insert("s_empty".into(), Value::String(String::new()));
    m.insert("other".into(), Value::Int(42));
    assert_eq!(
        get_datetime_str(&m, "dt").as_deref(),
        Some("2024-01-01T00:00:00")
    );
    assert_eq!(
        get_datetime_str(&m, "s_full").as_deref(),
        Some("2024-02-02")
    );
    assert!(get_datetime_str(&m, "s_empty").is_none());
    assert!(get_datetime_str(&m, "other").is_none());
    assert!(get_datetime_str(&m, "missing").is_none());
}

#[test]
fn get_str_array_returns_strings_only() {
    let mut m = BTreeMap::new();
    m.insert(
        "tags".into(),
        Value::Array(vec![
            Value::String("alpha".into()),
            Value::String("beta".into()),
            Value::Int(99),
        ]),
    );
    m.insert("not_array".into(), Value::String("oops".into()));
    assert_eq!(
        get_str_array(&m, "tags"),
        vec!["alpha".to_string(), "beta".to_string()]
    );
    assert!(get_str_array(&m, "not_array").is_empty());
    assert!(get_str_array(&m, "missing").is_empty());
}

#[test]
fn get_int_array_returns_ints_only() {
    let mut m = BTreeMap::new();
    m.insert(
        "blocks".into(),
        Value::Array(vec![
            Value::Int(42),
            Value::Int(100),
            Value::String("nope".into()),
        ]),
    );
    m.insert("not_array".into(), Value::Int(5));
    assert_eq!(get_int_array(&m, "blocks"), vec![42_u64, 100]);
    assert!(get_int_array(&m, "not_array").is_empty());
    assert!(get_int_array(&m, "missing").is_empty());
}
