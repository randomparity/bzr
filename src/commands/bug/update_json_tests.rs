#![expect(clippy::panic, clippy::unwrap_used)]

use std::collections::BTreeSet;

use crate::commands::runtime::from_json::JsonOneOrMany;

use super::JsonUpdateBug;

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
fn bug_update_input_schema_matches_parser_keys() {
    let schema_keys: BTreeSet<String> =
        schema_object_properties("bug-update-input", "bugUpdateInputObject")
            .keys()
            .cloned()
            .collect();
    let parser_keys = parser_fields_from_unknown_error::<JsonUpdateBug>();

    assert_eq!(schema_keys, parser_keys);
}

#[test]
fn bug_update_input_schema_examples_parse() {
    for (key, property_schema) in
        schema_object_properties("bug-update-input", "bugUpdateInputObject")
    {
        let example = property_schema
            .get("examples")
            .and_then(serde_json::Value::as_array)
            .and_then(|examples| examples.first())
            .unwrap_or_else(|| panic!("bug-update-input property '{key}' missing example"))
            .clone();
        let mut object = serde_json::Map::new();
        object.insert(key.clone(), example);

        serde_json::from_value::<JsonUpdateBug>(serde_json::Value::Object(object))
            .unwrap_or_else(|err| panic!("schema example for '{key}' did not parse: {err}"));
    }
}

#[test]
fn bug_update_input_schema_array_items_require_id() {
    let schema = schema_value("bug-update-input");
    let required = schema
        .pointer("/$defs/bugUpdateInputArrayItem/allOf/1/required")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("bug-update-input array item schema missing required list"));

    assert!(required.iter().any(|field| field.as_str() == Some("id")));
}

#[test]
fn parse_json_updates_object_and_array_shapes() {
    assert!(matches!(
        crate::commands::runtime::from_json::parse_one_or_many::<JsonUpdateBug>(
            r#"{"id":1,"status":"ASSIGNED"}"#
        )
        .unwrap(),
        JsonOneOrMany::One(_)
    ));

    match crate::commands::runtime::from_json::parse_one_or_many::<JsonUpdateBug>(
        r#"[{"id":1,"status":"ASSIGNED"},{"id":2,"priority":"high"}]"#,
    )
    .unwrap()
    {
        JsonOneOrMany::Many(v) => assert_eq!(v.len(), 2),
        JsonOneOrMany::One(_) => panic!("array should parse as Many"),
    }

    assert!(matches!(
        crate::commands::runtime::from_json::parse_one_or_many::<JsonUpdateBug>(
            r#"[{"id":1,"status":"ASSIGNED"}]"#
        )
        .unwrap(),
        JsonOneOrMany::Many(_)
    ));
}
