#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn flag_update_serializes_with_status_char() {
    let flag = FlagUpdate {
        name: "review".to_string(),
        status: FlagStatus::Grant,
        requestee: None,
    };
    let json = serde_json::to_value(&flag).unwrap();
    assert_eq!(json["name"], "review");
    assert_eq!(json["status"], "+");
    assert!(json.get("requestee").is_none());
}

#[test]
fn flag_update_serializes_with_requestee() {
    let flag = FlagUpdate {
        name: "needinfo".to_string(),
        status: FlagStatus::Request,
        requestee: Some("user@example.com".to_string()),
    };
    let json = serde_json::to_value(&flag).unwrap();
    assert_eq!(json["name"], "needinfo");
    assert_eq!(json["status"], "?");
    assert_eq!(json["requestee"], "user@example.com");
}

#[test]
fn flag_update_roundtrip() {
    let json = serde_json::json!({
        "name": "approval",
        "status": "-",
        "requestee": "admin@example.com"
    });
    let flag: FlagUpdate = serde_json::from_value(json).unwrap();
    assert_eq!(flag.name, "approval");
    assert_eq!(flag.status, FlagStatus::Deny);
    assert_eq!(flag.requestee.as_deref(), Some("admin@example.com"));
}

#[test]
fn flag_status_all_variants_roundtrip() {
    for (ch, expected) in [
        ("+", FlagStatus::Grant),
        ("-", FlagStatus::Deny),
        ("?", FlagStatus::Request),
        ("X", FlagStatus::Clear),
    ] {
        let json = serde_json::json!(ch);
        let status: FlagStatus = serde_json::from_value(json).unwrap();
        assert_eq!(status, expected);

        let serialized = serde_json::to_value(status).unwrap();
        assert_eq!(serialized, ch);
    }
}

#[test]
fn flag_status_display_emits_status_char() {
    for (status, expected) in [
        (FlagStatus::Grant, "+"),
        (FlagStatus::Deny, "-"),
        (FlagStatus::Request, "?"),
        (FlagStatus::Clear, "X"),
    ] {
        assert_eq!(status.to_string(), expected);
    }
}

#[test]
fn flag_status_invalid_deserialize() {
    let json = serde_json::json!("Z");
    let err = serde_json::from_value::<FlagStatus>(json).unwrap_err();
    assert!(err.to_string().contains("invalid flag status"));
}

#[test]
fn read_side_flag_missing_name_and_status_stay_unknown() {
    let flag: Flag = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(flag.name, None);
    assert_eq!(flag.status, None);

    let serialized = serde_json::to_value(flag).unwrap();
    assert!(serialized["name"].is_null());
    assert!(serialized["status"].is_null());
}

#[test]
fn read_side_flag_rendering_marks_missing_fields() {
    let flag = Flag {
        name: None,
        status: Some("+".into()),
        setter: None,
        requestee: Some("reviewer@example.com".into()),
    };

    assert_eq!(
        flag.render_inline(),
        "<missing-name>+(reviewer@example.com)"
    );
}
