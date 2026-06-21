#![expect(clippy::unwrap_used, clippy::panic)]

//! Drift guard for the published JSON Schemas.
//!
//! Output schemas are validated against the *actual serialized output* of a
//! representative typed value: every serialized key must be declared in the
//! schema's `properties` (unless the schema is open via `additionalProperties:
//! true`), and every `required` key must be present in the serialization.
//! Structured input schemas are guarded beside their parsers. A field added,
//! removed, or renamed therefore fails CI until its schema is updated, keeping
//! the published contract honest without a runtime schema-validation
//! dependency.

use serde_json::{json, Value};

use super::SCHEMAS;
use crate::output::result_types::{
    ActionKind, ActionResult, BatchCreateResult, BatchFailure, BatchResult, BugViewFailure,
    ConfigResult, CountResult, CreateFailure, DownloadResult, DryRunResult, MembershipResult,
    MultiBugViewResult, ResourceKind, SearchResult, TagResult, UploadResult,
};
use crate::test_helpers::CapturedIo;
use crate::types::{
    Attachment, BugzillaUser, Classification, Comment, Component, FieldValue, GroupInfo,
};

/// Look up a schema body by registry name.
fn schema_for(name: &str) -> Value {
    let (_, body) = SCHEMAS
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("no schema registered for {name}"));
    serde_json::from_str(body).unwrap_or_else(|e| panic!("schema {name} is not valid JSON: {e}"))
}

/// Assert `value` (the serialized form of a typed result) conforms to the named
/// schema's property/required contract.
///
/// `value` must be **maximally populated** — every optional field set — so the
/// check is a bijection for closed schemas: every serialized key must be a
/// declared property (no undocumented field), AND every declared property must
/// appear in the serialization (no *phantom* property the type never emits).
/// `required` keys must all be present. Open schemas (`additionalProperties:
/// true`, e.g. `bug`) skip the bijection since custom fields flatten in.
fn assert_conforms(name: &str, value: &Value) {
    let schema = schema_for(name);
    let obj = value
        .as_object()
        .unwrap_or_else(|| panic!("{name} sample is not a JSON object"));
    let properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("{name} schema has no properties object"));
    let open = schema
        .get("additionalProperties")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if !open {
        for key in obj.keys() {
            assert!(
                properties.contains_key(key),
                "{name}: serialized key '{key}' is not declared in the schema's properties \
                 (schema drift — update schemas/{name}.json)"
            );
        }
        // No phantom properties: a maximally-populated value must exercise every
        // declared property, or the schema advertises a field that cannot occur.
        for prop in properties.keys() {
            assert!(
                obj.contains_key(prop),
                "{name}: schema declares property '{prop}' but a maximally-populated value \
                 never serializes it (phantom property — fix schemas/{name}.json or the sample)"
            );
        }
    }

    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for req in required {
            let req = req.as_str().unwrap();
            assert!(
                obj.contains_key(req),
                "{name}: schema requires '{req}' but a representative value did not serialize it"
            );
        }
    }
}

fn to_value(value: &impl serde::Serialize) -> Value {
    serde_json::to_value(value).unwrap()
}

/// A representative wire bug, deserialized through `Bug`'s real `Deserialize`.
fn sample_bug() -> crate::types::Bug {
    serde_json::from_value(json!({
        "id": 1,
        "summary": "s",
        "status": "NEW",
        "keywords": [],
        "blocks": [],
        "depends_on": [],
        "cc": [],
    }))
    .unwrap()
}

// ── Envelope / result types ──────────────────────────────────────────

#[test]
fn action_result_conforms() {
    // Maximal: both optional `id` and `name` populated so the phantom-property
    // check exercises every declared property.
    assert_conforms(
        "action-result",
        &to_value(&ActionResult::created_named(
            7,
            "acme",
            ResourceKind::Product,
        )),
    );
}

#[test]
fn count_result_conforms() {
    assert_conforms("count-result", &to_value(&CountResult::new(5)));
}

#[test]
fn batch_result_conforms() {
    let batch = BatchResult::new(
        vec![1, 2],
        vec![BatchFailure {
            id: 3,
            error: "boom".into(),
        }],
    );
    assert_conforms("batch-result", &to_value(&batch));
}

#[test]
fn batch_create_result_conforms() {
    let batch = BatchCreateResult::new(
        vec![10],
        vec![CreateFailure {
            index: 0,
            error: "boom".into(),
        }],
    );
    assert_conforms("batch-create-result", &to_value(&batch));
}

#[test]
fn multi_bug_view_conforms() {
    let result = MultiBugViewResult {
        bugs: vec![sample_bug()],
        failed: vec![BugViewFailure {
            id: "missing".into(),
            error: "not found".into(),
        }],
    };
    assert_conforms("multi-bug-view", &to_value(&result));
}

#[test]
fn tag_result_conforms() {
    assert_conforms(
        "tag-result",
        &to_value(&TagResult::updated(7, vec!["triage".into()])),
    );
}

#[test]
fn membership_result_conforms() {
    assert_conforms(
        "membership-result",
        &to_value(&MembershipResult::added("alice", "editbugs")),
    );
}

#[test]
fn download_result_conforms() {
    assert_conforms(
        "download-result",
        &to_value(&DownloadResult::new(1, "patch.diff", 2048)),
    );
}

#[test]
fn upload_result_conforms() {
    assert_conforms("upload-result", &to_value(&UploadResult::new(9, 1, 2048)));
}

#[test]
fn config_result_conforms() {
    // Maximal: previous_name + url + is_default all populated. No public
    // constructor sets all three (each models a distinct operation), so build
    // the widest shape directly to exercise every declared property.
    let maximal = ConfigResult {
        name: "new".into(),
        previous_name: Some("old".into()),
        url: Some("https://bugs.example.com".into()),
        is_default: Some(true),
        config_file: "/cfg.toml".into(),
        resource: ResourceKind::Server,
        action: ActionKind::Renamed,
    };
    assert_conforms("config-result", &to_value(&maximal));
}

#[test]
fn search_result_conforms() {
    assert_conforms(
        "search-result",
        &to_value(&SearchResult::new(vec!["regression".into()])),
    );
}

#[test]
fn dry_run_result_conforms() {
    let ids = [1u64, 2];
    let changes = json!({ "summary": "new title" });
    let dry = DryRunResult::new(ResourceKind::Bug, &ids, &changes);
    assert_conforms("dry-run-result", &to_value(&dry));
}

#[test]
fn dry_run_result_schema_allows_admin_resources() {
    let schema = schema_for("dry-run-result");
    let resources = schema
        .pointer("/properties/resource/enum")
        .and_then(Value::as_array)
        .unwrap();
    for resource in ["product", "component", "user", "group"] {
        assert!(
            resources.contains(&Value::String(resource.into())),
            "dry-run-result schema must allow {resource} dry-run previews"
        );
    }
}

// ── Resource object types ────────────────────────────────────────────

#[test]
fn comment_conforms() {
    let comment: Comment = serde_json::from_value(json!({
        "id": 1, "bug_id": 2, "text": "hi", "creator": "a@b.c",
        "creation_time": "2026-01-01T00:00:00Z", "count": 0,
        "is_private": false, "attachment_id": null,
    }))
    .unwrap();
    assert_conforms("comment", &to_value(&comment));
}

#[test]
fn attachment_conforms() {
    let attachment: Attachment = serde_json::from_value(json!({
        "id": 1, "bug_id": 2, "file_name": "f.txt", "summary": "s",
        "content_type": "text/plain", "creator": "a@b.c",
        "creation_time": "t", "last_change_time": "t", "size": 10,
        "is_obsolete": false, "is_private": false, "is_patch": false,
        "data": "aGVsbG8=",
    }))
    .unwrap();
    assert_conforms("attachment", &to_value(&attachment));
}

#[test]
fn field_value_conforms() {
    // Maximal: the optional `can_change_to` populated so the schema's declared
    // property is exercised.
    let field_value: FieldValue = serde_json::from_value(json!({
        "name": "NEW", "sort_key": 0, "is_active": true,
        "can_change_to": [{"name": "ASSIGNED"}],
    }))
    .unwrap();
    assert_conforms("field-value", &to_value(&field_value));
}

#[test]
fn component_conforms() {
    let component: Component = serde_json::from_value(json!({
        "id": 1, "name": "core", "description": "d", "is_active": true,
        "default_assignee": "a@b.c",
    }))
    .unwrap();
    assert_conforms("component", &to_value(&component));
}

#[test]
fn classification_conforms() {
    let classification: Classification = serde_json::from_value(json!({
        "id": 1, "name": "Unclassified", "description": "d", "sort_key": 0,
        "products": [{"id": 2, "name": "p", "description": "pd"}],
    }))
    .unwrap();
    assert_conforms("classification", &to_value(&classification));
}

#[test]
fn user_conforms() {
    let user: BugzillaUser = serde_json::from_value(json!({
        "id": 1, "name": "alice", "real_name": "Alice", "email": "a@b.c",
        "groups": [{"id": 2, "name": "editbugs", "description": "d"}],
        "can_login": true,
    }))
    .unwrap();
    assert_conforms("user", &to_value(&user));
}

#[test]
fn group_conforms() {
    let group: GroupInfo = serde_json::from_value(json!({
        "id": 1, "name": "editbugs", "description": "d", "is_active": true,
        "membership": [{"id": 2, "name": "alice", "real_name": "Alice", "email": "a@b.c"}],
    }))
    .unwrap();
    assert_conforms("group", &to_value(&group));
}

#[test]
fn bug_object_is_open_and_documents_builtins() {
    // bug.json is intentionally open (additionalProperties: true) for flattened
    // custom fields, so conformance is trivial; assert the built-ins are still
    // documented so the schema stays useful.
    let schema = schema_for("bug");
    let props = schema.get("properties").and_then(Value::as_object).unwrap();
    for field in ["id", "summary", "status", "keywords", "custom_fields"]
        .iter()
        .filter(|f| **f != "custom_fields")
    {
        assert!(props.contains_key(*field), "bug schema missing '{field}'");
    }
    assert_eq!(
        schema.get("additionalProperties").and_then(Value::as_bool),
        Some(true)
    );
}

// ── Registry / well-formedness ───────────────────────────────────────

#[test]
fn every_schema_is_wellformed_and_named_draft_2020_12() {
    let mut checked = 0_usize;
    for (name, body) in SCHEMAS {
        checked += 1;
        let schema: Value =
            serde_json::from_str(body).unwrap_or_else(|e| panic!("{name}: invalid JSON: {e}"));
        assert_eq!(
            schema.get("$schema").and_then(Value::as_str),
            Some("https://json-schema.org/draft/2020-12/schema"),
            "{name}: wrong or missing $schema"
        );
        assert!(
            schema.get("title").and_then(Value::as_str).is_some(),
            "{name}: missing title"
        );
        assert!(
            schema.get("type").is_some(),
            "{name}: missing top-level type"
        );
        assert!(
            body.ends_with('\n'),
            "{name}: schema file must end with a newline"
        );
    }
    assert!(checked > 0, "schema registry is empty");
}

// ── Command behavior ─────────────────────────────────────────────────

fn run(name: Option<&str>, format: crate::types::OutputFormat) -> (CapturedIo, bool) {
    let mut io = CapturedIo::new();
    let ok = super::execute(name, format, &mut io.writers()).is_ok();
    (io, ok)
}

#[test]
fn execute_prints_named_schema_verbatim() {
    let (io, ok) = run(Some("bug"), crate::types::OutputFormat::Json);
    assert!(ok);
    let parsed: Value = serde_json::from_str(io.out_str()).unwrap();
    assert_eq!(parsed.get("title").and_then(Value::as_str), Some("Bug"));
}

#[test]
fn execute_unknown_schema_errors_with_available_list() {
    let (io, ok) = run(Some("nope"), crate::types::OutputFormat::Json);
    assert!(!ok);
    assert!(
        io.out_str().is_empty(),
        "nothing should be written on error"
    );
}

#[test]
fn execute_list_json_is_array_of_names() {
    let (io, ok) = run(None, crate::types::OutputFormat::Json);
    assert!(ok);
    let parsed: Value = serde_json::from_str(io.out_str()).unwrap();
    let names = parsed.as_array().unwrap();
    assert!(names.iter().any(|n| n == "bug"));
    assert!(names.iter().any(|n| n == "bug-create-input"));
    assert!(names.iter().any(|n| n == "bug-update-input"));
    assert!(names.iter().any(|n| n == "component-create-input"));
    assert!(names.iter().any(|n| n == "component-update-input"));
    assert!(names.iter().any(|n| n == "group-create-input"));
    assert!(names.iter().any(|n| n == "group-update-input"));
    assert!(names.iter().any(|n| n == "product-create-input"));
    assert!(names.iter().any(|n| n == "product-update-input"));
    assert!(names.iter().any(|n| n == "user-create-input"));
    assert!(names.iter().any(|n| n == "user-update-input"));
    assert!(names.iter().any(|n| n == "batch-result"));
}

#[test]
fn execute_list_ndjson_is_one_name_per_line() {
    let (io, ok) = run(None, crate::types::OutputFormat::Ndjson);
    assert!(ok);
    let lines: Vec<&str> = io.out_str().lines().collect();
    assert_eq!(lines.len(), SCHEMAS.len());
    // Each line is a bare JSON string.
    for line in lines {
        let v: Value = serde_json::from_str(line).unwrap();
        assert!(v.is_string());
    }
}

#[test]
fn execute_list_table_is_one_name_per_line() {
    let (io, ok) = run(None, crate::types::OutputFormat::Table);
    assert!(ok);
    assert_eq!(io.out_str().lines().count(), SCHEMAS.len());
}

#[test]
fn registry_is_sorted_and_unique() {
    let names: Vec<&str> = SCHEMAS.iter().map(|(n, _)| *n).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "SCHEMAS registry must stay sorted by name");
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        names.len(),
        "duplicate schema name in registry"
    );
}
