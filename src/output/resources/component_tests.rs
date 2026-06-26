#![expect(clippy::unwrap_used)]

use super::*;
use crate::types::Component;

fn make_component(id: u64, name: &str, active: bool) -> Component {
    Component {
        id,
        name: Some(name.into()),
        description: Some("A component".into()),
        is_active: Some(active),
        default_assignee: Some("dev@example.com".into()),
    }
}

#[test]
fn component_record_maps_fields() {
    let row = component_record(&make_component(7, "Backend", false));
    assert_eq!(row[0], "7");
    assert_eq!(row[1], "Backend");
    assert_eq!(row[3], "dev@example.com");
    assert_eq!(row[4], "No");
}

#[test]
fn component_record_dashes_missing_assignee() {
    let mut c = make_component(1, "X", true);
    c.default_assignee = None;
    let row = component_record(&c);
    assert_eq!(row[3], "-");
    assert_eq!(row[4], "Yes");
}

#[test]
fn write_components_table_lists_rows() {
    let items = vec![
        make_component(1, "Alpha", true),
        make_component(2, "Beta", true),
    ];
    let mut buf: Vec<u8> = Vec::new();
    write_components(&items, OutputFormat::Table, &mut buf);
    let out = String::from_utf8(buf).unwrap();
    assert!(out.contains("Alpha"));
    assert!(out.contains("Beta"));
    assert!(out.contains("NAME"));
}

#[test]
fn write_components_table_empty_message() {
    let items: Vec<Component> = vec![];
    let mut buf: Vec<u8> = Vec::new();
    write_components(&items, OutputFormat::Table, &mut buf);
    assert!(String::from_utf8(buf)
        .unwrap()
        .contains("No components found."));
}

#[test]
fn write_components_json_is_array() {
    let items = vec![make_component(3, "Gamma", true)];
    let mut buf: Vec<u8> = Vec::new();
    write_components(&items, OutputFormat::Json, &mut buf);
    let parsed: serde_json::Value =
        crate::test_helpers::json_envelope_data(std::str::from_utf8(&buf).unwrap());
    assert_eq!(parsed[0]["id"], 3);
    assert_eq!(parsed[0]["name"], "Gamma");
}

#[test]
fn write_component_table_shows_fields() {
    let mut buf: Vec<u8> = Vec::new();
    write_component(
        &make_component(9, "Core", false),
        OutputFormat::Table,
        &mut buf,
    );
    let out = String::from_utf8(buf).unwrap();
    assert!(out.contains("Core"));
    assert!(out.contains('9'));
    assert!(out.contains("No"));
}

#[test]
fn write_component_json_is_object() {
    let mut buf: Vec<u8> = Vec::new();
    write_component(
        &make_component(4, "Delta", true),
        OutputFormat::Json,
        &mut buf,
    );
    let parsed: serde_json::Value =
        crate::test_helpers::json_envelope_data(std::str::from_utf8(&buf).unwrap());
    assert_eq!(parsed["id"], 4);
    assert_eq!(parsed["name"], "Delta");
}

#[test]
fn write_component_table_omits_empty_description() {
    let mut c = make_component(5, "Empty", true);
    c.description = Some(String::new());
    let mut buf: Vec<u8> = Vec::new();
    write_component(&c, OutputFormat::Table, &mut buf);
    let out = String::from_utf8(buf).unwrap();
    assert!(out.contains("Empty"));
    assert!(
        !out.contains("Description"),
        "empty description must be omitted: {out}"
    );
}
