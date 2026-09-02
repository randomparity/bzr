#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn bugzilla_user_fields_matches_serialized_keys() {
    let user = BugzillaUser {
        id: 1,
        name: Some("alice".into()),
        real_name: Some("Alice".into()),
        email: Some("alice@example.com".into()),
        groups: vec![UserGroup {
            id: Some(1),
            name: Some("admin".into()),
            description: Some("Admins".into()),
        }],
        can_login: Some(true),
    };
    let value = serde_json::to_value(&user).unwrap();
    let serialized: std::collections::BTreeSet<String> =
        value.as_object().unwrap().keys().cloned().collect();
    let declared: std::collections::BTreeSet<String> = BUGZILLA_USER_FIELDS
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    assert_eq!(
        serialized, declared,
        "BUGZILLA_USER_FIELDS drifted from serde output"
    );
    assert_eq!(
        BUGZILLA_USER_FIELDS.len(),
        declared.len(),
        "BUGZILLA_USER_FIELDS has duplicates"
    );
}

#[test]
fn bugzilla_user_deserializes_full() {
    let json = serde_json::json!({
        "id": 123,
        "name": "alice",
        "real_name": "Alice Smith",
        "email": "alice@example.com",
        "can_login": true,
        "groups": [
            {"id": 1, "name": "admin", "description": "Admins"}
        ]
    });
    let user: BugzillaUser = serde_json::from_value(json).unwrap();
    assert_eq!(user.id, 123);
    assert_eq!(user.name.as_deref(), Some("alice"));
    assert_eq!(user.real_name.as_deref(), Some("Alice Smith"));
    assert_eq!(user.email.as_deref(), Some("alice@example.com"));
    assert_eq!(user.can_login, Some(true));
    assert_eq!(user.groups.len(), 1);
    assert_eq!(user.groups[0].name.as_deref(), Some("admin"));
}

#[test]
fn bugzilla_user_deserializes_production_shapes() {
    let user: BugzillaUser = serde_json::from_value(serde_json::json!({
        "id": "123",
        "can_login": 1,
        "groups": [{"id": "7", "name": "admin"}]
    }))
    .unwrap();

    assert_eq!(user.id, 123);
    assert_eq!(user.can_login, Some(true));
    assert_eq!(user.groups[0].id, Some(7));
}

#[test]
fn bugzilla_user_rejects_malformed_production_shapes() {
    for value in [
        serde_json::json!({"id": "-1"}),
        serde_json::json!({"id": 1, "can_login": 2}),
        serde_json::json!({"id": 1, "groups": [{"id": "1.5"}]}),
    ] {
        assert!(serde_json::from_value::<BugzillaUser>(value).is_err());
    }
}

#[test]
fn bugzilla_user_deserializes_minimal() {
    let json = serde_json::json!({"id": 1});
    let user: BugzillaUser = serde_json::from_value(json).unwrap();
    assert_eq!(user.id, 1);
    assert!(user.real_name.is_none());
    assert!(user.email.is_none());
    assert!(user.can_login.is_none());
    assert!(user.groups.is_empty());

    let serialized = serde_json::to_value(&user).unwrap();
    assert_eq!(serialized["name"], serde_json::Value::Null);
}

#[test]
fn user_group_missing_scalars_serialize_as_null() {
    let group: UserGroup = serde_json::from_value(serde_json::json!({})).unwrap();

    let serialized = serde_json::to_value(&group).unwrap();
    assert_eq!(serialized["id"], serde_json::Value::Null);
    assert_eq!(serialized["name"], serde_json::Value::Null);
    assert_eq!(serialized["description"], serde_json::Value::Null);
}

#[test]
fn whoami_response_deserializes() {
    let json = serde_json::json!({
        "id": 42,
        "name": "bob",
        "real_name": "Bob Jones",
        "login": "bob@example.com"
    });
    let whoami: WhoamiResponse = serde_json::from_value(json).unwrap();
    assert_eq!(whoami.id, 42);
    assert_eq!(whoami.name.as_deref(), Some("bob"));
    assert_eq!(whoami.real_name.as_deref(), Some("Bob Jones"));
    assert_eq!(whoami.login.as_deref(), Some("bob@example.com"));
}

#[test]
fn whoami_response_deserializes_string_id() {
    let whoami: WhoamiResponse = serde_json::from_value(serde_json::json!({"id": "42"})).unwrap();
    assert_eq!(whoami.id, 42);
}

#[test]
fn whoami_from_bugzilla_user() {
    let user = BugzillaUser {
        id: 99,
        name: Some("carol".to_string()),
        real_name: Some("Carol White".to_string()),
        email: Some("carol@example.com".to_string()),
        groups: vec![],
        can_login: Some(true),
    };
    let whoami = WhoamiResponse::from(user);
    assert_eq!(whoami.id, 99);
    assert_eq!(whoami.name.as_deref(), Some("carol"));
    assert_eq!(whoami.real_name.as_deref(), Some("Carol White"));
    assert_eq!(whoami.login.as_deref(), Some("carol@example.com"));
}

#[test]
fn whoami_from_user_maps_email_to_login() {
    let user = BugzillaUser {
        id: 1,
        name: Some("test".to_string()),
        real_name: None,
        email: None,
        groups: vec![],
        can_login: None,
    };
    let whoami = WhoamiResponse::from(user);
    assert!(whoami.login.is_none());
}

#[test]
fn whoami_output_flattens_identity_with_connection_metadata() {
    let output = WhoamiOutput {
        identity: WhoamiResponse {
            id: 42,
            name: Some("bob".into()),
            real_name: Some("Bob Jones".into()),
            login: Some("bob@example.com".into()),
        },
        server_name: "prod".into(),
        auth_mode: crate::types::AuthMode::ApiKey,
    };
    let serialized = serde_json::to_value(&output).unwrap();
    // Identity keys are flattened to the top level, not nested under `identity`.
    assert!(serialized.get("identity").is_none());
    assert_eq!(serialized["id"], 42);
    assert_eq!(serialized["name"], "bob");
    assert_eq!(serialized["real_name"], "Bob Jones");
    assert_eq!(serialized["login"], "bob@example.com");
    assert_eq!(serialized["server_name"], "prod");
    assert_eq!(serialized["auth_mode"], "api_key");
}

#[test]
fn whoami_output_serializes_absent_identity_fields_as_null() {
    let output = WhoamiOutput {
        identity: WhoamiResponse {
            id: 1,
            name: Some("bot".into()),
            real_name: None,
            login: None,
        },
        server_name: "(inline)".into(),
        auth_mode: crate::types::AuthMode::Anonymous,
    };
    let serialized = serde_json::to_value(&output).unwrap();
    assert_eq!(serialized["real_name"], serde_json::Value::Null);
    assert_eq!(serialized["login"], serde_json::Value::Null);
    assert_eq!(serialized["server_name"], "(inline)");
    assert_eq!(serialized["auth_mode"], "anonymous");
}
