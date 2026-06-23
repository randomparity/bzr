use std::collections::BTreeMap;

use super::{
    get_datetime_str, get_flags, get_int_array, get_nonempty_str, get_str_array, require_u64,
};
use crate::error::BzrError;
use crate::xmlrpc::protocol::Value;

#[test]
fn get_flags_parses_structs_and_skips_non_structs() {
    let mut flag = BTreeMap::new();
    flag.insert("name".into(), Value::String("review".into()));
    flag.insert("status".into(), Value::String("+".into()));
    flag.insert("setter".into(), Value::String("alice@example.com".into()));

    let mut m = BTreeMap::new();
    m.insert(
        "flags".into(),
        Value::Array(vec![Value::Struct(flag), Value::Int(7)]),
    );

    let flags = get_flags(&m, "flags");
    assert_eq!(flags.len(), 1, "non-struct element should be skipped");
    assert_eq!(flags[0].name, "review");
    assert_eq!(flags[0].status, "+");
    assert_eq!(flags[0].setter.as_deref(), Some("alice@example.com"));
    assert!(flags[0].requestee.is_none());

    assert!(get_flags(&m, "missing").is_empty());
}

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
fn require_u64_rejects_negative_id() {
    // XML-RPC transmits integers as signed i64, but a domain identifier can
    // never be negative. The bit pattern must NOT wrap into a huge u64 (the old
    // `as u64` cast); it must surface as a malformed response that names the
    // field and shows the offending value.
    let mut m = BTreeMap::new();
    m.insert("id".into(), Value::Int(-1));
    let result = require_u64(&m, "id", "bug");
    assert!(
        matches!(&result, Err(BzrError::XmlRpc(msg)) if msg.contains("negative") && msg.contains("-1")),
        "negative id should be a malformed-response error, got: {result:?}"
    );
}

#[test]
fn require_u64_missing_field_is_distinct_from_negative() {
    // The absent-field branch reports "missing", not "negative", so the operator
    // can tell a truncated response from a bad value.
    let empty = BTreeMap::new();
    let result = require_u64(&empty, "id", "comment");
    assert!(
        matches!(&result, Err(BzrError::XmlRpc(msg)) if msg.contains("missing") && msg.contains("comment")),
        "missing id should report a missing field, got: {result:?}"
    );
}

#[test]
fn require_u64_accepts_non_negative() {
    let mut m = BTreeMap::new();
    m.insert("id".into(), Value::Int(42));
    assert!(matches!(require_u64(&m, "id", "bug"), Ok(42)));
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
