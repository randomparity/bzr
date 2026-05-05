#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn group_info_deserializes_full() {
    let json = serde_json::json!({
        "id": 42,
        "name": "admin",
        "description": "Administrators",
        "is_active": true,
        "membership": [
            {
                "id": 1,
                "name": "alice",
                "real_name": "Alice Smith",
                "email": "alice@example.com"
            }
        ]
    });
    let group: GroupInfo = serde_json::from_value(json).unwrap();
    assert_eq!(group.id, 42);
    assert_eq!(group.name, "admin");
    assert_eq!(group.description, "Administrators");
    assert!(group.is_active);
    assert_eq!(group.membership.len(), 1);
    assert_eq!(group.membership[0].name, "alice");
    assert_eq!(
        group.membership[0].real_name.as_deref(),
        Some("Alice Smith")
    );
}

#[test]
fn group_info_deserializes_minimal() {
    let json = serde_json::json!({"id": 7});
    let group: GroupInfo = serde_json::from_value(json).unwrap();
    assert_eq!(group.id, 7);
    assert_eq!(group.name, "");
    assert_eq!(group.description, "");
    assert!(!group.is_active);
    assert!(group.membership.is_empty());
}

#[test]
fn group_member_deserializes_without_optional_fields() {
    let json = serde_json::json!({"id": 99, "name": "bob"});
    let member: GroupMember = serde_json::from_value(json).unwrap();
    assert_eq!(member.id, 99);
    assert_eq!(member.name, "bob");
    assert!(member.real_name.is_none());
    assert!(member.email.is_none());
}
