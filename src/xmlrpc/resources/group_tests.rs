#![expect(clippy::unwrap_used)]

use std::collections::BTreeMap;

use super::value_to_group_info;
use crate::xmlrpc::protocol::Value;

#[test]
fn value_to_group_info_parses_membership_and_optional_fields() {
    let mut member = BTreeMap::new();
    member.insert("id".into(), Value::Int(7));
    member.insert("name".into(), Value::String("alice@example.com".into()));
    member.insert("real_name".into(), Value::String("Alice".into()));
    member.insert("email".into(), Value::String("alice@example.com".into()));

    let mut group = BTreeMap::new();
    group.insert("id".into(), Value::Int(1));
    group.insert("name".into(), Value::String("admin".into()));
    group.insert("description".into(), Value::String("Administrators".into()));
    group.insert("is_active".into(), Value::Bool(true));
    group.insert(
        "membership".into(),
        Value::Array(vec![Value::Struct(member)]),
    );

    let info = value_to_group_info(&Value::Struct(group)).unwrap();
    assert_eq!(info.name.as_deref(), Some("admin"));
    assert_eq!(info.is_active, Some(true));
    assert_eq!(info.membership.len(), 1);
    assert_eq!(info.membership[0].id, 7);
    assert_eq!(info.membership[0].real_name.as_deref(), Some("Alice"));
}

#[test]
fn value_to_group_info_parses_int_is_active() {
    let mut group = BTreeMap::new();
    group.insert("id".into(), Value::Int(1));
    group.insert("name".into(), Value::String("admin".into()));
    group.insert("description".into(), Value::String("Administrators".into()));
    group.insert("is_active".into(), Value::Int(1));
    group.insert("membership".into(), Value::Array(Vec::new()));

    let info = value_to_group_info(&Value::Struct(group)).unwrap();
    assert_eq!(info.is_active, Some(true));
}
