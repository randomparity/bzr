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
    CompoundCreateResult, ConfigResult, CountResult, CreateFailure, DownloadResult, DryRunResult,
    MembershipResult, MultiBugViewResult, ResourceKind, SearchResult, SubStepFailure, TagResult,
    UploadResult,
};
use crate::test_helpers::CapturedIo;
use crate::types::{
    Attachment, AuthMode, BugAdjacencyBug, BugAdjacencyError, BugAdjacencyRequest,
    BugAdjacencyResult, BugzillaUser, Classification, Comment, Component, CustomFieldSummary,
    FieldValue, FlagTypeSummary, GroupInfo, HistoryRecord, ServerCapabilities,
    StatusTransitionSummary, WhoamiOutput, WhoamiResponse,
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

fn schema_accepts(name: &str, value: &Value) -> bool {
    let schema = schema_for(name);
    value_matches_schema(&schema, value, &schema)
}

fn value_matches_schema(schema: &Value, value: &Value, root: &Value) -> bool {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let pointer = reference.strip_prefix('#').unwrap();
        return value_matches_schema(root.pointer(pointer).unwrap(), value, root);
    }
    if let Some(one_of) = schema.get("oneOf").and_then(Value::as_array) {
        return one_of
            .iter()
            .filter(|variant| value_matches_schema(variant, value, root))
            .count()
            == 1;
    }
    scalar_constraints_match(schema, value)
        && array_constraints_match(schema, value, root)
        && object_constraints_match(schema, value, root)
}

fn scalar_constraints_match(schema: &Value, value: &Value) -> bool {
    if schema
        .get("const")
        .is_some_and(|expected| value != expected)
        || schema
            .get("enum")
            .and_then(Value::as_array)
            .is_some_and(|variants| !variants.contains(value))
    {
        return false;
    }
    let right_type = match schema.get("type").and_then(Value::as_str) {
        Some("object") => value.is_object(),
        Some("array") => value.is_array(),
        Some("string") => value.is_string(),
        Some("integer") => value.as_i64().is_some() || value.as_u64().is_some(),
        Some("boolean") => value.is_boolean(),
        Some("null") => value.is_null(),
        Some(_) => false,
        None => true,
    };
    if !right_type {
        return false;
    }
    if let Some(text) = value.as_str() {
        if let Some(minimum) = schema.get("minLength").and_then(Value::as_u64) {
            if text.chars().count() < usize::try_from(minimum).unwrap() {
                return false;
            }
        }
    }
    if let Some(number) = value
        .as_i64()
        .map(i128::from)
        .or_else(|| value.as_u64().map(i128::from))
    {
        if schema
            .get("minimum")
            .and_then(Value::as_i64)
            .is_some_and(|minimum| number < i128::from(minimum))
            || schema
                .get("maximum")
                .and_then(Value::as_u64)
                .is_some_and(|maximum| number > i128::from(maximum))
        {
            return false;
        }
    }
    true
}

fn array_constraints_match(schema: &Value, value: &Value, root: &Value) -> bool {
    if let Some(items) = schema.get("items") {
        if !value.as_array().is_none_or(|values| {
            values
                .iter()
                .all(|item| value_matches_schema(items, item, root))
        }) {
            return false;
        }
    }
    true
}

fn object_constraints_match(schema: &Value, value: &Value, root: &Value) -> bool {
    if let Some(object) = value.as_object() {
        let properties = schema.get("properties").and_then(Value::as_object);
        if schema
            .get("required")
            .and_then(Value::as_array)
            .is_some_and(|required| {
                required
                    .iter()
                    .filter_map(Value::as_str)
                    .any(|key| !object.contains_key(key))
            })
        {
            return false;
        }
        if schema.get("additionalProperties") == Some(&Value::Bool(false))
            && object
                .keys()
                .any(|key| properties.is_none_or(|known| !known.contains_key(key)))
        {
            return false;
        }
        if properties.is_some_and(|known| {
            object.iter().any(|(key, item)| {
                known
                    .get(key)
                    .is_some_and(|item_schema| !value_matches_schema(item_schema, item, root))
            })
        }) {
            return false;
        }
    }
    true
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
        vec![
            BatchFailure::new(3, "boom"),
            BatchFailure::comment_tags(4, "not found"),
        ],
    );
    assert_conforms("batch-result", &to_value(&batch));
}

#[test]
fn batch_create_result_conforms() {
    let batch = BatchCreateResult::new(
        vec![10, 11],
        vec![
            CreateFailure::create(0, "boom"),
            CreateFailure::sub_step(2, 11, "attachment", Some("t.log".into()), "500"),
        ],
    );
    assert_conforms("batch-create-result", &to_value(&batch));
}

#[test]
fn compound_create_result_conforms() {
    let result = CompoundCreateResult::new(
        42,
        vec![
            SubStepFailure::comment("comment service unavailable"),
            SubStepFailure::attachment("trace.log", "413 payload too large"),
            SubStepFailure::comment_tags("comment not found"),
        ],
    );
    assert_conforms("compound-create-result", &to_value(&result));
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
fn bug_adjacency_maximal_success_conforms() {
    let maximal = u64::try_from(i64::MAX).unwrap();
    let result = BugAdjacencyResult {
        requests: vec![BugAdjacencyRequest::Success {
            requested: "release/2026".into(),
            bug_id: maximal,
        }],
        bugs: vec![BugAdjacencyBug {
            id: maximal,
            summary: Some("summary".into()),
            status: Some("NEW".into()),
            resolution: Some("FIXED".into()),
            product: Some("Product".into()),
            version: Some(vec!["1.0".into()]),
            assigned_to: Some("owner@example.invalid".into()),
            last_change_time: Some("2026-08-29T00:00:00Z".into()),
            target_milestone: Some("---".into()),
            blocks: vec![0, maximal],
            depends_on: vec![1],
        }],
    };
    let value = to_value(&result);
    assert_conforms("bug-adjacency", &value);
    assert!(schema_accepts("bug-adjacency", &value));
}

#[test]
fn bug_adjacency_nullable_scalars_and_failure_variants_conform() {
    let nullable = json!({
        "requests": [{"requested": "1", "bug_id": 1}],
        "bugs": [{
            "id": 1,
            "summary": null,
            "status": null,
            "resolution": null,
            "product": null,
            "version": null,
            "assigned_to": null,
            "last_change_time": null,
            "target_milestone": null,
            "blocks": [],
            "depends_on": []
        }]
    });
    assert!(schema_accepts("bug-adjacency", &nullable));

    for error in [
        BugAdjacencyError::NotFoundAlias,
        BugAdjacencyError::NotFoundId,
        BugAdjacencyError::Inaccessible,
    ] {
        let result = BugAdjacencyResult {
            requests: vec![BugAdjacencyRequest::Failure {
                requested: "missing".into(),
                error,
            }],
            bugs: vec![],
        };
        assert!(schema_accepts("bug-adjacency", &to_value(&result)));
    }
}

#[test]
fn bug_adjacency_schema_rejects_invalid_failure_pairings_and_empty_strings() {
    for value in [
        json!({
            "requests": [{
                "requested": "missing",
                "error": {"type": "inaccessible", "api_code": 100}
            }],
            "bugs": []
        }),
        json!({
            "requests": [{
                "requested": "missing",
                "error": {"type": "not_found", "api_code": 102}
            }],
            "bugs": []
        }),
        json!({"requests": [{"requested": "", "bug_id": 1}], "bugs": []}),
        json!({
            "requests": [{"requested": "1", "bug_id": 1}],
            "bugs": [{
                "id": 1,
                "summary": "",
                "status": null,
                "resolution": null,
                "product": null,
                "version": null,
                "assigned_to": null,
                "last_change_time": null,
                "target_milestone": null,
                "blocks": [],
                "depends_on": []
            }]
        }),
    ] {
        assert!(!schema_accepts("bug-adjacency", &value), "accepted {value}");
    }
}

#[test]
fn bug_adjacency_schema_rejects_undeclared_keys_and_out_of_range_ids() {
    let maximal = u64::try_from(i64::MAX).unwrap();
    for value in [
        json!({"requests": [], "bugs": [], "extra": true}),
        json!({"requests": [{"requested": "1", "bug_id": 1, "extra": true}], "bugs": []}),
        json!({"requests": [{"requested": "1", "bug_id": maximal + 1}], "bugs": []}),
        json!({"requests": [{"requested": "1", "bug_id": -1}], "bugs": []}),
        json!({
            "requests": [],
            "bugs": [{
                "id": 1,
                "summary": null,
                "status": null,
                "resolution": null,
                "product": null,
                "version": null,
                "assigned_to": null,
                "last_change_time": null,
                "target_milestone": null,
                "blocks": [maximal + 1],
                "depends_on": []
            }]
        }),
    ] {
        assert!(!schema_accepts("bug-adjacency", &value), "accepted {value}");
    }
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
        "tags": ["needs-info", "follow-up"],
    }))
    .unwrap();
    assert_conforms("comment", &to_value(&comment));
}

#[test]
fn comment_schema_accepts_tags() {
    let comment = json!({
        "id": 1, "bug_id": 2, "text": "hi", "creator": "a@b.c",
        "creation_time": "2026-01-01T00:00:00Z", "count": 0,
        "is_private": false, "attachment_id": null,
        "tags": ["needs-info", "follow-up"],
    });
    assert!(schema_accepts("comment", &comment));
}

#[test]
fn comment_schema_requires_tags() {
    let comment = json!({
        "id": 1, "bug_id": 2, "text": "hi", "creator": "a@b.c",
        "creation_time": "2026-01-01T00:00:00Z", "count": 0,
        "is_private": false, "attachment_id": null,
    });
    assert!(!schema_accepts("comment", &comment));
}

#[test]
fn comment_schema_rejects_non_array_tags() {
    let comment = json!({
        "id": 1, "bug_id": 2, "text": "hi", "creator": "a@b.c",
        "creation_time": "2026-01-01T00:00:00Z", "count": 0,
        "is_private": false, "attachment_id": null,
        "tags": "needs-info",
    });
    assert!(!schema_accepts("comment", &comment));
}

#[test]
fn comment_schema_rejects_non_string_tag() {
    let comment = json!({
        "id": 1, "bug_id": 2, "text": "hi", "creator": "a@b.c",
        "creation_time": "2026-01-01T00:00:00Z", "count": 0,
        "is_private": false, "attachment_id": null,
        "tags": ["needs-info", 7],
    });
    assert!(!schema_accepts("comment", &comment));
}

#[test]
fn history_record_conforms() {
    // Maximal: comment_id populated so the closed-schema bijection exercises
    // every declared property.
    let record = HistoryRecord {
        when: "2026-06-01T14:22:01Z".into(),
        who: "alice@example.com".into(),
        field: "status".into(),
        old_value: Some("NEW".into()),
        new_value: Some("ASSIGNED".into()),
        comment_id: Some(42),
    };
    assert_conforms("history", &to_value(&record));
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
fn metadata_sort_key_schemas_are_nullable_bounded_integers() {
    let field = schema_for("field-value");
    let product = schema_for("product");
    let nodes = [
        field.pointer("/properties/sort_key").unwrap(),
        product
            .pointer("/properties/versions/items/properties/sort_key")
            .unwrap(),
        product
            .pointer("/properties/milestones/items/properties/sort_key")
            .unwrap(),
    ];

    let accepts = |node: &Value, value: &Value| {
        let types = node.get("type").and_then(Value::as_array).unwrap();
        let has_type = (value.is_null() && types.contains(&json!("null")))
            || ((value.as_i64().is_some() || value.as_u64().is_some())
                && types.contains(&json!("integer")));
        let number = value
            .as_i64()
            .map(i128::from)
            .or_else(|| value.as_u64().map(i128::from));
        has_type
            && number.is_none_or(|number| {
                number >= i128::from(node["minimum"].as_i64().unwrap())
                    && number <= i128::from(node["maximum"].as_u64().unwrap())
            })
    };

    for node in nodes {
        assert_eq!(node["type"], json!(["integer", "null"]));
        assert_eq!(node["minimum"], json!(i64::MIN));
        assert_eq!(node["maximum"], json!(u64::MAX));
        for value in [Value::Null, json!(i64::MIN), json!(u64::MAX)] {
            assert!(accepts(node, &value));
        }

        let mut wrong_type = node.clone();
        wrong_type["type"] = json!(["string", "null"]);
        assert!(!accepts(&wrong_type, &json!(-1)));
    }
}

#[test]
fn server_capabilities_conforms() {
    // Maximal: every nullable field (max_attachment_size, flag_types) populated so
    // the closed-schema bijection exercises all declared properties.
    let caps = ServerCapabilities {
        version: "5.0.4".to_string(),
        api_modes: vec!["rest".to_string(), "xmlrpc".to_string()],
        auth_modes: vec!["api_key".to_string()],
        max_attachment_size: Some(1_024_000),
        status_transitions: vec![StatusTransitionSummary {
            from: "NEW".to_string(),
            can_change_to: vec!["ASSIGNED".to_string()],
        }],
        flag_types: Some(vec![FlagTypeSummary {
            name: "review".to_string(),
            requestable: true,
            multiplicable: false,
        }]),
        custom_fields: vec![CustomFieldSummary {
            name: "cf_release".to_string(),
            field_type: "single_select".to_string(),
            values: vec!["1.0".to_string()],
        }],
        supports_comments: true,
        supports_attachments: true,
        supports_history: true,
        supports_flag_requests: true,
    };
    assert_conforms("server-capabilities", &to_value(&caps));
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
fn whoami_conforms() {
    // Maximal: every nullable identity field populated so the closed-schema
    // bijection exercises all declared properties.
    let output = WhoamiOutput {
        identity: WhoamiResponse {
            id: 1,
            name: Some("alice".into()),
            real_name: Some("Alice".into()),
            login: Some("alice@example.com".into()),
        },
        server_name: "prod".into(),
        auth_mode: AuthMode::ApiKey,
    };
    assert_conforms("whoami", &to_value(&output));
}

#[test]
fn whoami_schema_constrains_auth_mode_enum() {
    let schema = schema_for("whoami");
    let variants = schema
        .pointer("/properties/auth_mode/enum")
        .and_then(Value::as_array)
        .unwrap();
    for mode in ["api_key", "anonymous"] {
        assert!(
            variants.contains(&Value::String(mode.into())),
            "whoami schema auth_mode enum must allow {mode}"
        );
    }
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
    for field in [
        "id",
        "summary",
        "status",
        "keywords",
        "groups",
        "estimated_time",
        "remaining_time",
        "custom_fields",
    ]
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

async fn run(name: Option<&str>, format: crate::types::OutputFormat) -> (CapturedIo, bool) {
    let mut io = CapturedIo::new();
    let ok = super::execute(
        name,
        &crate::commands::runtime::invocation::CommandContext::new(None, format, None),
        &mut io.writers(),
    )
    .await
    .is_ok();
    (io, ok)
}

#[tokio::test]
async fn execute_prints_named_schema_verbatim() {
    let (io, ok) = run(Some("bug"), crate::types::OutputFormat::Json).await;
    assert!(ok);
    let parsed: Value = serde_json::from_str(io.out_str()).unwrap();
    assert_eq!(parsed.get("title").and_then(Value::as_str), Some("Bug"));
}

#[tokio::test]
async fn execute_named_schema_is_not_enveloped() {
    // `bzr schema <name>` emits the raw JSON-Schema document verbatim — it must
    // NOT be wrapped in the `{schema_version, data}` envelope (which would
    // corrupt the published schema document).
    let (io, ok) = run(Some("bug"), crate::types::OutputFormat::Json).await;
    assert!(ok);
    let parsed: Value = serde_json::from_str(io.out_str()).unwrap();
    assert!(
        parsed.get("schema_version").is_none(),
        "named schema document must not carry the envelope"
    );
    assert!(parsed.get("data").is_none());
    assert!(
        parsed.get("$schema").is_some(),
        "should be a raw schema doc"
    );
}

#[tokio::test]
async fn execute_unknown_schema_errors_with_available_list() {
    let (io, ok) = run(Some("nope"), crate::types::OutputFormat::Json).await;
    assert!(!ok);
    assert!(
        io.out_str().is_empty(),
        "nothing should be written on error"
    );
}

#[tokio::test]
async fn execute_list_json_is_array_of_names() {
    let (io, ok) = run(None, crate::types::OutputFormat::Json).await;
    assert!(ok);
    // `bzr schema` (list) flows through the success seam, so it is enveloped:
    // `{schema_version, data: [...names]}`.
    let parsed = crate::test_helpers::json_envelope_data(io.out_str());
    let names = parsed.as_array().unwrap();
    assert!(names.iter().any(|n| n == "bug"));
    assert!(names.iter().any(|n| n == "bug-adjacency"));
    assert!(names.iter().any(|n| n == "bug-create-input"));
    assert!(names.iter().any(|n| n == "bug-update-input"));
    assert!(names.iter().any(|n| n == "component-create-input"));
    assert!(!names.iter().any(|n| n == "component-update-input"));
    assert!(names.iter().any(|n| n == "error"));
    assert!(names.iter().any(|n| n == "group-create-input"));
    assert!(names.iter().any(|n| n == "group-update-input"));
    assert!(names.iter().any(|n| n == "product-create-input"));
    assert!(names.iter().any(|n| n == "product-update-input"));
    assert!(names.iter().any(|n| n == "user-create-input"));
    assert!(names.iter().any(|n| n == "user-update-input"));
    assert!(names.iter().any(|n| n == "batch-result"));
}

#[tokio::test]
async fn execute_list_ndjson_is_one_name_per_line() {
    let (io, ok) = run(None, crate::types::OutputFormat::Ndjson).await;
    assert!(ok);
    let lines: Vec<&str> = io.out_str().lines().collect();
    assert_eq!(lines.len(), SCHEMAS.len());
    // Each line is a bare JSON string.
    for line in lines {
        let v: Value = serde_json::from_str(line).unwrap();
        assert!(v.is_string());
    }
}

#[tokio::test]
async fn execute_list_table_is_one_name_per_line() {
    let (io, ok) = run(None, crate::types::OutputFormat::Table).await;
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
