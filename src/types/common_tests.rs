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
fn output_format_from_str_valid() {
    assert_eq!(
        "table".parse::<OutputFormat>().unwrap(),
        OutputFormat::Table
    );
    assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
    assert_eq!(
        "ndjson".parse::<OutputFormat>().unwrap(),
        OutputFormat::Ndjson
    );
}

#[test]
fn output_format_is_json_family() {
    assert!(OutputFormat::Json.is_json_family());
    assert!(OutputFormat::Ndjson.is_json_family());
    assert!(!OutputFormat::Table.is_json_family());
}

#[test]
fn output_format_from_str_invalid() {
    let err = "xml".parse::<OutputFormat>().unwrap_err();
    assert!(err.contains("invalid output format"));
    assert!(err.contains("ndjson"));
}

#[test]
fn output_format_default_is_table() {
    assert_eq!(OutputFormat::default(), OutputFormat::Table);
}

#[test]
fn server_version_deserializes() {
    let json = serde_json::json!({"version": "5.0.4"});
    let ver: ServerVersion = serde_json::from_value(json).unwrap();
    assert_eq!(ver.version, "5.0.4");
}

#[test]
fn server_extensions_deserializes() {
    let json = serde_json::json!({
        "extensions": {
            "MyExtension": {"version": "1.0"},
            "Bare": {}
        }
    });
    let ext: ServerExtensions = serde_json::from_value(json).unwrap();
    assert_eq!(ext.extensions.len(), 2);
    assert_eq!(
        ext.extensions["MyExtension"].version.as_deref(),
        Some("1.0")
    );
    assert!(ext.extensions["Bare"].version.is_none());
}

#[test]
fn auth_method_from_str() {
    assert_eq!("header".parse::<AuthMethod>().unwrap(), AuthMethod::Header);
    assert_eq!(
        "query_param".parse::<AuthMethod>().unwrap(),
        AuthMethod::QueryParam
    );
    assert!("bogus".parse::<AuthMethod>().is_err());
}

#[test]
fn api_mode_from_str() {
    assert_eq!("rest".parse::<ApiMode>().unwrap(), ApiMode::Rest);
    assert_eq!("xmlrpc".parse::<ApiMode>().unwrap(), ApiMode::XmlRpc);
    assert_eq!("hybrid".parse::<ApiMode>().unwrap(), ApiMode::Hybrid);
    assert!("grpc".parse::<ApiMode>().is_err());
}

#[test]
fn sort_direction_from_str_and_keyword() {
    assert_eq!("asc".parse::<SortDirection>().unwrap(), SortDirection::Asc);
    assert_eq!("ASC".parse::<SortDirection>().unwrap(), SortDirection::Asc);
    assert_eq!(
        "ascending".parse::<SortDirection>().unwrap(),
        SortDirection::Asc
    );
    assert_eq!(
        "desc".parse::<SortDirection>().unwrap(),
        SortDirection::Desc
    );
    assert_eq!(
        "DESCENDING".parse::<SortDirection>().unwrap(),
        SortDirection::Desc
    );
    assert!("sideways".parse::<SortDirection>().is_err());
    assert_eq!(SortDirection::Asc.keyword(), "ASC");
    assert_eq!(SortDirection::Desc.keyword(), "DESC");
    assert_eq!(SortDirection::default(), SortDirection::Asc);
}
