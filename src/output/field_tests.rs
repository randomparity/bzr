#![expect(clippy::unwrap_used)]

use super::{write_field_aliases, write_field_values};
use crate::types::{FieldValue, OutputFormat, StatusTransition};

fn capture_values(format: OutputFormat, values: &[FieldValue]) -> String {
    let mut buf = Vec::new();
    write_field_values(values, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

fn capture_aliases(format: OutputFormat, aliases: &[(&'static str, &'static str)]) -> String {
    let mut buf = Vec::new();
    write_field_aliases(aliases, format, &mut buf);
    String::from_utf8(buf).unwrap()
}

#[test]
fn write_field_values_json_empty() {
    let values: Vec<FieldValue> = vec![];
    let json = serde_json::to_string_pretty(&values).unwrap();
    assert_eq!(json, "[]");
}

#[test]
fn write_field_values_json_with_transitions() {
    let values = vec![FieldValue {
        name: "NEW".into(),
        sort_key: 0,
        is_active: true,
        can_change_to: Some(vec![
            StatusTransition {
                name: "ASSIGNED".into(),
            },
            StatusTransition {
                name: "RESOLVED".into(),
            },
        ]),
    }];
    let json = serde_json::to_string_pretty(&values).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed[0]["name"], "NEW");
    assert_eq!(parsed[0]["is_active"], true);
    let transitions = parsed[0]["can_change_to"].as_array().unwrap();
    assert_eq!(transitions.len(), 2);
    assert_eq!(transitions[0]["name"], "ASSIGNED");
}

#[test]
fn write_field_values_json_active_and_inactive() {
    let values = vec![
        FieldValue {
            name: "NEW".into(),
            sort_key: 0,
            is_active: true,
            can_change_to: Some(vec![StatusTransition {
                name: "ASSIGNED".into(),
            }]),
        },
        FieldValue {
            name: "CLOSED".into(),
            sort_key: 1,
            is_active: false,
            can_change_to: None,
        },
    ];
    let json = serde_json::to_string_pretty(&values).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed[0]["name"], "NEW");
    assert_eq!(parsed[0]["is_active"], true);
    assert_eq!(parsed[1]["name"], "CLOSED");
    assert_eq!(parsed[1]["is_active"], false);
    assert!(parsed[1]["can_change_to"].is_null());
}

#[test]
fn write_field_aliases_json_serialization() {
    let aliases: &[(&str, &str)] = &[("status", "bug_status"), ("severity", "bug_severity")];
    let rows: Vec<super::FieldAliasRow> = aliases
        .iter()
        .map(|&(alias, api_name)| super::FieldAliasRow { alias, api_name })
        .collect();
    let json = serde_json::to_string_pretty(&rows).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0]["alias"], "status");
    assert_eq!(arr[0]["api_name"], "bug_status");
    assert_eq!(arr[1]["alias"], "severity");
    assert_eq!(arr[1]["api_name"], "bug_severity");
}

#[test]
fn write_field_values_table_empty_renders_only_headers() {
    let output = capture_values(OutputFormat::Table, &[]);
    assert!(output.contains("NAME"));
    assert!(output.contains("ACTIVE"));
    assert!(output.contains("CAN CHANGE TO"));
}

#[test]
fn write_field_values_json_empty_renders_empty_array() {
    let output = capture_values(OutputFormat::Json, &[]);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert!(parsed.is_array());
    assert_eq!(parsed.as_array().unwrap().len(), 0);
}

#[test]
fn write_field_values_table_renders_transitions_and_inactive() {
    let values = vec![
        FieldValue {
            name: "NEW".into(),
            sort_key: 0,
            is_active: true,
            can_change_to: Some(vec![
                StatusTransition {
                    name: "ASSIGNED".into(),
                },
                StatusTransition {
                    name: "RESOLVED".into(),
                },
            ]),
        },
        FieldValue {
            name: "CLOSED".into(),
            sort_key: 1,
            is_active: false,
            can_change_to: None,
        },
    ];
    let output = capture_values(OutputFormat::Table, &values);
    assert!(output.contains("NEW"));
    assert!(output.contains("CLOSED"));
    assert!(output.contains("Yes"));
    assert!(output.contains("No"));
    assert!(output.contains("ASSIGNED, RESOLVED"));
}

#[test]
fn write_field_values_table_handles_unicode_value_name() {
    let values = vec![FieldValue {
        name: "résolu".into(),
        sort_key: 0,
        is_active: true,
        can_change_to: Some(vec![StatusTransition {
            name: "fermé".into(),
        }]),
    }];
    let output = capture_values(OutputFormat::Table, &values);
    assert!(output.contains("résolu"));
    assert!(output.contains("fermé"));
}

#[test]
fn write_field_values_json_via_write() {
    let values = vec![FieldValue {
        name: "NEW".into(),
        sort_key: 0,
        is_active: true,
        can_change_to: Some(vec![StatusTransition {
            name: "ASSIGNED".into(),
        }]),
    }];
    let output = capture_values(OutputFormat::Json, &values);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed[0]["name"], "NEW");
    assert_eq!(parsed[0]["is_active"], true);
}

#[test]
fn write_field_aliases_table_renders_rows() {
    let aliases: &[(&'static str, &'static str)] =
        &[("status", "bug_status"), ("severity", "bug_severity")];
    let output = capture_aliases(OutputFormat::Table, aliases);
    assert!(output.contains("ALIAS"));
    assert!(output.contains("API FIELD NAME"));
    assert!(output.contains("status"));
    assert!(output.contains("bug_status"));
    assert!(output.contains("severity"));
    assert!(output.contains("bug_severity"));
}

#[test]
fn write_field_aliases_json_via_write() {
    let aliases: &[(&'static str, &'static str)] = &[("status", "bug_status")];
    let output = capture_aliases(OutputFormat::Json, aliases);
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed[0]["alias"], "status");
    assert_eq!(parsed[0]["api_name"], "bug_status");
}
