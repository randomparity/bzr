#![expect(clippy::unwrap_used)]

use super::{
    api_modes_for, auth_modes_for, field_type_name, supports_rest_surface, CustomFieldSummary,
    FlagTypeSummary, ServerCapabilities, StatusTransitionSummary,
};
use crate::types::ApiMode;

#[test]
fn field_type_names_map_every_known_code() {
    assert_eq!(field_type_name(1), "freetext");
    assert_eq!(field_type_name(2), "single_select");
    assert_eq!(field_type_name(3), "multi_select");
    assert_eq!(field_type_name(4), "textarea");
    assert_eq!(field_type_name(5), "datetime");
    assert_eq!(field_type_name(6), "bug_id");
    assert_eq!(field_type_name(7), "bug_urls");
    assert_eq!(field_type_name(8), "keywords");
    assert_eq!(field_type_name(9), "date");
    assert_eq!(field_type_name(10), "integer");
}

#[test]
fn field_type_name_unknown_for_zero_and_out_of_range() {
    assert_eq!(field_type_name(0), "unknown");
    assert_eq!(field_type_name(99), "unknown");
    assert_eq!(field_type_name(-1), "unknown");
}

#[test]
fn api_modes_match_transport() {
    assert_eq!(api_modes_for(ApiMode::Rest), vec!["rest".to_string()]);
    assert_eq!(
        api_modes_for(ApiMode::Hybrid),
        vec!["rest".to_string(), "xmlrpc".to_string()]
    );
    assert_eq!(api_modes_for(ApiMode::XmlRpc), vec!["xmlrpc".to_string()]);
}

#[test]
fn auth_modes_present_for_rest_surface_absent_for_xmlrpc() {
    assert_eq!(auth_modes_for(ApiMode::Rest), vec!["api_key".to_string()]);
    assert_eq!(auth_modes_for(ApiMode::Hybrid), vec!["api_key".to_string()]);
    assert!(auth_modes_for(ApiMode::XmlRpc).is_empty());
}

#[test]
fn rest_surface_true_for_rest_and_hybrid_only() {
    assert!(supports_rest_surface(ApiMode::Rest));
    assert!(supports_rest_surface(ApiMode::Hybrid));
    assert!(!supports_rest_surface(ApiMode::XmlRpc));
}

#[test]
fn serializes_all_keys_with_null_and_renamed_type() {
    let caps = ServerCapabilities {
        version: "5.0.4".to_string(),
        api_modes: api_modes_for(ApiMode::Rest),
        auth_modes: auth_modes_for(ApiMode::Rest),
        max_attachment_size: None,
        status_transitions: vec![StatusTransitionSummary {
            from: "NEW".to_string(),
            can_change_to: vec!["ASSIGNED".to_string()],
        }],
        flag_types: None,
        custom_fields: vec![CustomFieldSummary {
            name: "cf_x".to_string(),
            field_type: "freetext".to_string(),
            values: Vec::new(),
        }],
        supports_comments: true,
        supports_attachments: true,
        supports_history: true,
        supports_flag_requests: true,
    };

    let value = serde_json::to_value(&caps).unwrap();

    // Nullable keys must be present (not omitted) so agents can branch on value.
    assert!(value
        .as_object()
        .unwrap()
        .contains_key("max_attachment_size"));
    assert!(value["max_attachment_size"].is_null());
    assert!(value.as_object().unwrap().contains_key("flag_types"));
    assert!(value["flag_types"].is_null());

    // Custom field type serializes under the JSON key `type`, not `field_type`.
    assert_eq!(value["custom_fields"][0]["type"], "freetext");
    assert!(value["custom_fields"][0]
        .as_object()
        .unwrap()
        .get("field_type")
        .is_none());

    assert_eq!(value["version"], "5.0.4");
    assert_eq!(value["api_modes"][0], "rest");
    assert_eq!(value["status_transitions"][0]["from"], "NEW");
    assert_eq!(value["supports_flag_requests"], true);
}

#[test]
fn flag_type_summary_serializes_expected_shape() {
    let flag_type = FlagTypeSummary {
        name: "review".to_string(),
        requestable: true,
        multiplicable: false,
    };

    let value = serde_json::to_value(&flag_type).unwrap();

    assert_eq!(value["name"], "review");
    assert_eq!(value["requestable"], true);
    assert_eq!(value["multiplicable"], false);
}
