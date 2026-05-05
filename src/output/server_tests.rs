#![expect(clippy::unwrap_used)]

use crate::types::{ServerExtensions, ServerVersion};

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
