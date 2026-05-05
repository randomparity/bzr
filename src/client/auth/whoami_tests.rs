#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn whoami_response_deserializes_with_id() {
    let json = r#"{"id": 42, "name": "user@example.com"}"#;
    let resp: WhoamiProbeResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.id, 42);
}

#[test]
fn whoami_response_id_zero_means_unauthenticated() {
    let json = r#"{"id": 0}"#;
    let resp: WhoamiProbeResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.id, 0);
}

#[test]
fn whoami_response_missing_id_defaults_zero() {
    let json = r#"{"name": "test"}"#;
    let resp: WhoamiProbeResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.id, 0);
}
