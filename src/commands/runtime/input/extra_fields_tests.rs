#![expect(clippy::unwrap_used)]

use serde_json::json;

use super::{parse_extra_fields, reject_typed_collisions};

fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_string()).collect()
}

fn write_json(tmp: &tempfile::TempDir, contents: &str) -> String {
    let path = tmp.path().join("fields.json");
    std::fs::write(&path, contents).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn pairs_become_string_values_in_key_order() {
    let fields = parse_extra_fields(
        &args(["cf_release=9.6", "whiteboard=text"].as_slice()),
        None,
    )
    .expect("valid pairs parse");
    let keys: Vec<&str> = fields.keys().map(String::as_str).collect();
    assert_eq!(keys, ["cf_release", "whiteboard"]);
    assert_eq!(fields["cf_release"], json!("9.6"));
    assert_eq!(fields["whiteboard"], json!("text"));
}

#[test]
fn value_keeps_every_character_after_the_first_separator() {
    let fields = parse_extra_fields(&args(["cf_expr=a=b=c"].as_slice()), None).unwrap();
    assert_eq!(fields["cf_expr"], json!("a=b=c"));
}

/// `--field key=` is how a field is cleared on Bugzilla; it must survive as an
/// empty string rather than being rejected as missing.
#[test]
fn empty_value_clears_the_field() {
    let fields = parse_extra_fields(&args(["cf_release="].as_slice()), None).unwrap();
    assert_eq!(fields["cf_release"], json!(""));
}

#[test]
fn pair_without_separator_is_rejected() {
    let err = parse_extra_fields(&args(["cf_release"].as_slice()), None).unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(
        err.to_string().contains("is not KEY=VALUE"),
        "message should name the shape: {err}"
    );
}

#[test]
fn empty_key_is_rejected() {
    let err = parse_extra_fields(&args(["  =value"].as_slice()), None).unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(err.to_string().contains("empty field name"), "{err}");
}

#[test]
fn duplicate_pair_key_is_rejected_rather_than_resolved() {
    let err = parse_extra_fields(&args(["cf_a=1", "cf_a=2"].as_slice()), None).unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(err.to_string().contains("more than once"), "{err}");
    assert!(err.to_string().contains("cf_a"), "{err}");
}

#[test]
fn json_source_preserves_value_types() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = write_json(
        &tmp,
        r#"{"cf_multi": ["a", "b"], "cf_flag": true, "cf_count": 3}"#,
    );
    let fields = parse_extra_fields(&[], Some(&source)).unwrap();
    assert_eq!(fields["cf_multi"], json!(["a", "b"]));
    assert_eq!(fields["cf_flag"], json!(true));
    assert_eq!(fields["cf_count"], json!(3));
}

#[test]
fn key_supplied_by_both_sources_is_rejected() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = write_json(&tmp, r#"{"cf_a": "from-json"}"#);
    let err = parse_extra_fields(&args(["cf_a=from-flag"].as_slice()), Some(&source)).unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(err.to_string().contains("more than once"), "{err}");
}

#[test]
fn json_source_must_be_an_object() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = write_json(&tmp, r#"["cf_a"]"#);
    let err = parse_extra_fields(&[], Some(&source)).unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(
        err.to_string().contains("must contain a JSON object"),
        "{err}"
    );
}

#[test]
fn malformed_json_names_the_source() {
    let tmp = tempfile::TempDir::new().unwrap();
    let source = write_json(&tmp, "{not json");
    let err = parse_extra_fields(&[], Some(&source)).unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(err.to_string().contains(&source), "{err}");
}

#[test]
fn unreadable_json_source_is_rejected() {
    let tmp = tempfile::TempDir::new().unwrap();
    let missing = tmp.path().join("absent.json");
    let err = parse_extra_fields(&[], Some(&missing.to_string_lossy())).unwrap_err();
    assert_eq!(err.exit_code(), 7);
}

#[test]
fn empty_inputs_produce_an_empty_map() {
    assert!(parse_extra_fields(&[], None).unwrap().is_empty());
}

#[test]
fn collision_with_a_serialized_typed_field_is_rejected() {
    let typed = json!({"product": "P", "whiteboard": "set"});
    let extra = parse_extra_fields(&args(["whiteboard=other"].as_slice()), None).unwrap();
    let err = reject_typed_collisions(&typed, &extra).unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(err.to_string().contains("dedicated flag"), "{err}");
    assert!(err.to_string().contains("whiteboard"), "{err}");
}

/// The check reads the *serialized* payload, so a field the typed path skipped
/// stays available to `--field`. This is the case the python-bugzilla
/// comparison harness drives.
#[test]
fn key_absent_from_the_serialized_payload_is_allowed() {
    let typed = json!({"product": "P", "component": "C"});
    let extra = parse_extra_fields(&args(["whiteboard=text"].as_slice()), None).unwrap();
    assert!(reject_typed_collisions(&typed, &extra).is_ok());
}

#[test]
fn collision_check_is_a_no_op_without_extras() {
    let typed = json!({"product": "P"});
    let extra = parse_extra_fields(&[], None).unwrap();
    assert!(reject_typed_collisions(&typed, &extra).is_ok());
}
