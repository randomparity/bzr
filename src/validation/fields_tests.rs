#![expect(clippy::unwrap_used)]

use serde_json::json;

use crate::validation::fields::FieldProjection;

const KNOWN: &[&str] = &["id", "name", "description", "is_active"];

#[test]
fn include_keeps_only_named_keys() {
    let proj = FieldProjection::resolve(Some("id,name"), None, KNOWN).unwrap();
    let mut v = json!({"id": 1, "name": "x", "description": "y", "is_active": true});
    proj.apply(&mut v);
    assert_eq!(v, json!({"id": 1, "name": "x"}));
}

#[test]
fn exclude_drops_named_keys() {
    let proj = FieldProjection::resolve(None, Some("description,is_active"), KNOWN).unwrap();
    let mut v = json!({"id": 1, "name": "x", "description": "y", "is_active": true});
    proj.apply(&mut v);
    assert_eq!(v, json!({"id": 1, "name": "x"}));
}

#[test]
fn neither_is_identity() {
    let proj = FieldProjection::resolve(None, None, KNOWN).unwrap();
    let mut v = json!({"id": 1, "name": "x"});
    proj.apply(&mut v);
    assert_eq!(v, json!({"id": 1, "name": "x"}));
    assert!(!proj.is_requested());
}

#[test]
fn requested_flag_set_when_include_given() {
    let proj = FieldProjection::resolve(Some("id"), None, KNOWN).unwrap();
    assert!(proj.is_requested());
}

#[test]
fn unknown_include_token_errors() {
    let err = FieldProjection::resolve(Some("id,bogus"), None, KNOWN).unwrap_err();
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn unknown_exclude_token_errors() {
    let err = FieldProjection::resolve(None, Some("bogus"), KNOWN).unwrap_err();
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn exclude_every_key_errors() {
    let err =
        FieldProjection::resolve(None, Some("id,name,description,is_active"), KNOWN).unwrap_err();
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn blank_include_is_identity() {
    let proj = FieldProjection::resolve(Some(" , ,"), None, KNOWN).unwrap();
    let mut v = json!({"id": 1, "name": "x"});
    proj.apply(&mut v);
    assert_eq!(v, json!({"id": 1, "name": "x"}));
}

#[test]
fn combined_subtracts_exclude_from_include() {
    let proj = FieldProjection::resolve(Some("id,name"), Some("id"), KNOWN).unwrap();
    let mut v = json!({"id": 1, "name": "x"});
    proj.apply(&mut v);
    assert_eq!(v, json!({"name": "x"}));
}

#[test]
fn combined_exclude_absent_from_include_is_inert() {
    let proj = FieldProjection::resolve(Some("id,name"), Some("description"), KNOWN).unwrap();
    let mut v = json!({"id": 1, "name": "x", "description": "y"});
    proj.apply(&mut v);
    assert_eq!(v, json!({"id": 1, "name": "x"}));
}

#[test]
fn combined_empty_result_errors() {
    let err = FieldProjection::resolve(Some("id"), Some("id"), KNOWN).unwrap_err();
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn apply_projects_every_array_element() {
    let proj = FieldProjection::resolve(Some("id"), None, KNOWN).unwrap();
    let mut v = json!([{"id": 1, "name": "a"}, {"id": 2, "name": "b"}]);
    proj.apply(&mut v);
    assert_eq!(v, json!([{"id": 1}, {"id": 2}]));
}

#[test]
fn apply_on_scalar_is_noop() {
    let proj = FieldProjection::resolve(Some("id"), None, KNOWN).unwrap();
    let mut v = json!("hello");
    proj.apply(&mut v);
    assert_eq!(v, json!("hello"));
}

#[test]
fn apply_absent_key_yields_sparse_object() {
    let proj = FieldProjection::resolve(Some("description"), None, KNOWN).unwrap();
    let mut v = json!({"id": 1, "name": "x"});
    proj.apply(&mut v);
    assert_eq!(v, json!({}));
}

#[test]
fn projected_key_order_follows_serialization_not_request_order() {
    let proj = FieldProjection::resolve(Some("name,id"), None, KNOWN).unwrap();
    let mut v = json!({"id": 1, "name": "x"});
    proj.apply(&mut v);
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(keys, vec!["id", "name"]);
}

#[test]
fn projection_for_table_warns_and_returns_identity() {
    use crate::types::output::OutputFormat;
    let mut err = Vec::new();
    let proj = crate::validation::fields::projection_for(
        OutputFormat::Table,
        Some("id"),
        None,
        KNOWN,
        &mut err,
    )
    .unwrap();
    assert!(!proj.is_requested());
    let warning = String::from_utf8(err).unwrap();
    assert!(warning.contains("--fields/--exclude-fields only affect"));
}

#[test]
fn projection_for_json_validates() {
    use crate::types::output::OutputFormat;
    let mut err = Vec::new();
    let result = crate::validation::fields::projection_for(
        OutputFormat::Json,
        Some("bogus"),
        None,
        KNOWN,
        &mut err,
    );
    assert_eq!(result.unwrap_err().exit_code(), 7);
}
