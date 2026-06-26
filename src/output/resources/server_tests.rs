#![expect(clippy::unwrap_used)]

use crate::types::{CustomFieldSummary, OutputFormat, ServerCapabilities, StatusTransitionSummary};
use crate::types::{ServerExtensions, ServerVersion};

fn sample_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        version: "5.0.4".to_string(),
        api_modes: vec!["rest".to_string()],
        auth_modes: vec!["api_key".to_string()],
        max_attachment_size: Some(1_024_000),
        status_transitions: vec![StatusTransitionSummary {
            from: "NEW".to_string(),
            can_change_to: vec!["ASSIGNED".to_string(), "RESOLVED".to_string()],
        }],
        flag_types: None,
        custom_fields: vec![CustomFieldSummary {
            name: "cf_release".to_string(),
            field_type: "single_select".to_string(),
            values: vec!["1.0".to_string()],
        }],
        supports_comments: true,
        supports_attachments: true,
        supports_history: true,
        supports_flag_requests: true,
    }
}

#[test]
fn capabilities_json_round_trips_documented_shape() {
    let caps = sample_capabilities();
    let mut out: Vec<u8> = Vec::new();
    super::write_server_capabilities(&caps, OutputFormat::Json, &mut out);

    let parsed: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(parsed["version"], "5.0.4");
    assert_eq!(parsed["api_modes"][0], "rest");
    assert_eq!(parsed["max_attachment_size"], 1_024_000);
    assert!(parsed.as_object().unwrap().contains_key("flag_types"));
    assert!(parsed["flag_types"].is_null());
    assert_eq!(parsed["custom_fields"][0]["type"], "single_select");
    assert_eq!(parsed["supports_flag_requests"], true);
}

#[test]
fn capabilities_table_renders_key_fields() {
    let caps = sample_capabilities();
    let mut out: Vec<u8> = Vec::new();
    super::write_server_capabilities(&caps, OutputFormat::Table, &mut out);

    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("5.0.4"));
    assert!(text.contains("rest"));
    assert!(text.contains("NEW"));
    assert!(text.contains("cf_release"));
    assert!(text.contains("1024000"));
}

#[test]
fn capabilities_table_shows_unknown_for_absent_attachment_size() {
    let mut caps = sample_capabilities();
    caps.max_attachment_size = None;
    let mut out: Vec<u8> = Vec::new();
    super::write_server_capabilities(&caps, OutputFormat::Table, &mut out);

    let text = String::from_utf8(out).unwrap();
    assert!(text.to_lowercase().contains("unknown"));
}

#[test]
fn server_info_text_format_with_extensions() {
    let info = super::ServerInfo {
        version: "5.0.4",
        extensions: &{
            let mut m = std::collections::HashMap::new();
            m.insert(
                "BmpConvert".into(),
                crate::types::ExtensionInfo {
                    version: Some("1.0".into()),
                },
            );
            m
        },
    };
    assert_eq!(info.version, "5.0.4");
    assert!(info.extensions.contains_key("BmpConvert"));
}

#[test]
fn server_info_text_format_no_extensions() {
    let empty = std::collections::HashMap::new();
    let info = super::ServerInfo {
        version: "4.0",
        extensions: &empty,
    };
    assert!(info.extensions.is_empty());
}

#[test]
fn print_server_info_json_combined() {
    let version = ServerVersion {
        version: "5.0.4".into(),
    };
    let extensions = ServerExtensions {
        extensions: {
            let mut m = std::collections::HashMap::new();
            m.insert(
                "BmpConvert".into(),
                crate::types::ExtensionInfo {
                    version: Some("1.0".into()),
                },
            );
            m
        },
    };
    let combined = serde_json::json!({
        "version": version.version,
        "extensions": extensions.extensions,
    });
    let json = serde_json::to_string_pretty(&combined).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["version"], "5.0.4");
    assert!(parsed["extensions"]["BmpConvert"].is_object());
}
