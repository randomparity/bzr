#![expect(clippy::unwrap_used, clippy::panic)]

use std::collections::BTreeSet;

use crate::error::BzrError;

use super::JsonCreateBug;

fn schema_value(name: &str) -> serde_json::Value {
    let (_, body) = crate::commands::schema::SCHEMAS
        .iter()
        .find(|(schema_name, _)| *schema_name == name)
        .unwrap_or_else(|| panic!("schema '{name}' is not registered"));
    serde_json::from_str(body).unwrap_or_else(|err| panic!("schema '{name}' is invalid: {err}"))
}

fn schema_object_properties(
    name: &str,
    def_name: &str,
) -> serde_json::Map<String, serde_json::Value> {
    let pointer = format!("/$defs/{def_name}/properties");
    schema_value(name)
        .pointer(&pointer)
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("schema '{name}' missing {pointer}"))
        .clone()
}

fn parser_fields_from_unknown_error<T>() -> BTreeSet<String>
where
    T: serde::de::DeserializeOwned,
{
    let err = match serde_json::from_value::<T>(serde_json::json!({
        "__bzr_unknown_field__": true,
    })) {
        Ok(_) => panic!("unknown sentinel field unexpectedly parsed"),
        Err(err) => err.to_string(),
    };
    let expected = err
        .split_once("expected ")
        .unwrap_or_else(|| panic!("serde error did not list expected fields: {err}"))
        .1;
    expected
        .split('`')
        .skip(1)
        .step_by(2)
        .map(ToString::to_string)
        .collect()
}

#[test]
fn bug_create_input_schema_matches_parser_keys() {
    let schema_keys: BTreeSet<String> =
        schema_object_properties("bug-create-input", "bugCreateInputObject")
            .keys()
            .cloned()
            .collect();
    let parser_keys = parser_fields_from_unknown_error::<JsonCreateBug>();

    assert_eq!(schema_keys, parser_keys);
}

#[test]
fn bug_create_input_schema_examples_parse() {
    for (key, property_schema) in
        schema_object_properties("bug-create-input", "bugCreateInputObject")
    {
        let example = property_schema
            .get("examples")
            .and_then(serde_json::Value::as_array)
            .and_then(|examples| examples.first())
            .unwrap_or_else(|| panic!("bug-create-input property '{key}' missing example"))
            .clone();
        let mut object = serde_json::Map::new();
        object.insert(key.clone(), example);

        serde_json::from_value::<JsonCreateBug>(serde_json::Value::Object(object))
            .unwrap_or_else(|err| panic!("schema example for '{key}' did not parse: {err}"));
    }
}

#[test]
fn bug_create_input_schema_documents_array_form() {
    let schema = schema_value("bug-create-input");
    let one_of = schema
        .get("oneOf")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("bug-create-input schema missing oneOf"));
    let array_branch = one_of
        .iter()
        .find(|branch| branch.get("type").and_then(serde_json::Value::as_str) == Some("array"))
        .unwrap_or_else(|| panic!("bug-create-input schema does not document array input"));

    assert_eq!(
        array_branch
            .get("minItems")
            .and_then(serde_json::Value::as_u64),
        Some(1)
    );
}

#[test]
fn parse_json_bugs_rejects_non_object_scalar() {
    let err = super::parse_json_bugs("42").unwrap_err();
    assert!(matches!(err, BzrError::InputValidation(_)));
}

#[test]
fn parse_json_bugs_rejects_malformed_json() {
    let err = super::parse_json_bugs("{not json").unwrap_err();
    match err {
        BzrError::InputValidation(msg) => assert!(msg.contains("invalid JSON"), "{msg}"),
        other => panic!("expected InputValidation, got {other:?}"),
    }
}

#[test]
fn parse_json_bugs_object_and_array_shapes() {
    assert!(matches!(
        super::parse_json_bugs(r#"{"product":"P"}"#).unwrap(),
        super::JsonInput::One(_)
    ));
    match super::parse_json_bugs(r#"[{"product":"P"},{"product":"Q"}]"#).unwrap() {
        super::JsonInput::Many(v) => assert_eq!(v.len(), 2),
        super::JsonInput::One(_) => panic!("array should parse as Many"),
    }
    // A 1-element array stays Many, so output shape follows input shape.
    assert!(matches!(
        super::parse_json_bugs(r#"[{"product":"P"}]"#).unwrap(),
        super::JsonInput::Many(_)
    ));
}
