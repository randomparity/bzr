#![expect(clippy::unwrap_used)]

use super::*;

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
