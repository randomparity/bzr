#![expect(clippy::unwrap_used, clippy::panic)]

use super::*;

#[test]
fn bug_deserializes_minimal() {
    let json = r#"{"id": 42}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();
    assert_eq!(bug.id, 42);
    assert!(bug.summary.is_empty());
    assert!(bug.keywords.is_empty());
    assert!(bug.custom_fields.is_empty());
}

#[test]
fn bug_deserializes_full() {
    let json = r#"{"id": 1, "summary": "test bug", "status": "NEW", "product": "Core", "component": "General", "priority": "P1", "keywords": ["regression"]}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();
    assert_eq!(bug.summary, "test bug");
    assert_eq!(bug.status, "NEW");
    assert_eq!(bug.product.as_deref(), Some("Core"));
    assert_eq!(bug.keywords, vec!["regression"]);
}

#[test]
fn bug_deserializes_deadline() {
    let json = r#"{"id": 42, "deadline": "2026-12-31"}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();
    let serialized = serde_json::to_value(&bug).unwrap();

    assert_eq!(serialized["deadline"], "2026-12-31");
}

#[test]
fn bug_deserializes_target_milestone_and_flags() {
    let json = r#"{
        "id": 42,
        "target_milestone": "9.0",
        "flags": [
            {"name": "review", "status": "+", "setter": "alice@example.com"},
            {"name": "needinfo", "status": "?", "requestee": "bob@example.com"}
        ]
    }"#;
    let bug: Bug = serde_json::from_str(json).unwrap();

    assert_eq!(bug.target_milestone.as_deref(), Some("9.0"));
    assert_eq!(bug.flags.len(), 2);
    assert_eq!(bug.flags[0].name, "review");
    assert_eq!(bug.flags[0].status, "+");
    assert_eq!(bug.flags[0].setter.as_deref(), Some("alice@example.com"));
    assert_eq!(bug.flags[1].status, "?");
    assert_eq!(bug.flags[1].requestee.as_deref(), Some("bob@example.com"));
}

#[test]
fn bug_without_flags_defaults_to_empty_and_serializes_array() {
    let bug: Bug = serde_json::from_str(r#"{"id": 42}"#).unwrap();
    assert!(bug.flags.is_empty());
    assert!(bug.target_milestone.is_none());

    let serialized = serde_json::to_value(&bug).unwrap();
    // flags is always present as an array (empty -> []), target_milestone null.
    assert_eq!(serialized["flags"], serde_json::json!([]));
    assert!(serialized["target_milestone"].is_null());
}

#[test]
fn bug_serializes_flags_and_target_milestone() {
    let json =
        r#"{"id": 7, "target_milestone": "---", "flags": [{"name": "review", "status": "+"}]}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();
    let serialized = serde_json::to_value(&bug).unwrap();

    // JSON stays faithful: the raw "---" sentinel is preserved (only the table
    // detail suppresses it).
    assert_eq!(serialized["target_milestone"], "---");
    assert_eq!(serialized["flags"][0]["name"], "review");
    assert_eq!(serialized["flags"][0]["status"], "+");
}

#[test]
fn flag_with_unexpected_status_token_still_deserializes() {
    // The read-side Flag.status is a plain String, so a token the FlagStatus
    // enum does not model must not break bug view.
    let json = r#"{"id": 1, "flags": [{"name": "weird", "status": "??"}]}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();
    assert_eq!(bug.flags[0].status, "??");
}

#[test]
fn bug_deserializes_custom_fields() {
    let json = r#"{"id": 42, "summary": "s", "cf_release": "9.6"}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();

    assert_eq!(bug.custom_fields["cf_release"], "9.6");
}

#[test]
fn bug_deserializes_sparse_custom_fields_with_defaults() {
    let json = r#"{"id": 42, "cf_release": "9.6"}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();

    assert_eq!(bug.id, 42);
    assert!(bug.summary.is_empty());
    assert!(bug.status.is_empty());
    assert!(bug.keywords.is_empty());
    assert_eq!(bug.custom_fields["cf_release"], "9.6");
}

#[test]
fn bug_deserialization_drops_non_custom_extension_keys() {
    let json = r#"{"id": 42, "x_extension": "ignored", "cf_release": "9.6"}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();

    assert!(!bug.custom_fields.contains_key("x_extension"));
    assert!(bug.custom_fields.contains_key("cf_release"));
}

#[test]
fn bug_serializes_custom_fields_as_top_level_keys() {
    let mut bug: Bug = serde_json::from_str(r#"{"id": 42}"#).unwrap();
    bug.custom_fields
        .insert("cf_release".into(), serde_json::json!("9.6"));

    let serialized = serde_json::to_value(&bug).unwrap();

    assert_eq!(serialized["cf_release"], "9.6");
    assert!(serialized.get("custom_fields").is_none());
}

#[test]
fn bug_serialization_drops_non_custom_entries_from_public_map() {
    let mut bug: Bug = serde_json::from_str(r#"{"id": 42}"#).unwrap();
    bug.custom_fields
        .insert("cf_release".into(), serde_json::json!("9.6"));
    bug.custom_fields
        .insert("x_extension".into(), serde_json::json!("ignored"));

    let serialized = serde_json::to_value(&bug).unwrap();

    assert_eq!(serialized["cf_release"], "9.6");
    assert!(serialized.get("x_extension").is_none());
}

#[test]
fn bug_serializes_custom_fields_after_built_ins_sorted_by_name() {
    let mut bug: Bug = serde_json::from_str(r#"{"id": 42, "summary": "s"}"#).unwrap();
    bug.custom_fields
        .insert("cf_zeta".into(), serde_json::json!("z"));
    bug.custom_fields
        .insert("cf_alpha".into(), serde_json::json!("a"));

    let serialized = serde_json::to_value(&bug).unwrap();
    let keys: Vec<&str> = serialized
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();

    assert_eq!(&keys[0..3], ["id", "summary", "status"]);
    assert_eq!(&keys[keys.len() - 2..], ["cf_alpha", "cf_zeta"]);
}

#[test]
fn partition_filters_positive_only() {
    let vals: Vec<String> = vec!["NEW".into(), "ASSIGNED".into()];
    let (pos, neg) = partition_filters(&vals);
    assert_eq!(pos, vec!["NEW", "ASSIGNED"]);
    assert!(neg.is_empty());
}

#[test]
fn partition_filters_negated_only() {
    let vals: Vec<String> = vec!["!CLOSED".into(), "!VERIFIED".into()];
    let (pos, neg) = partition_filters(&vals);
    assert!(pos.is_empty());
    assert_eq!(neg, vec!["CLOSED", "VERIFIED"]);
}

#[test]
fn partition_filters_mixed() {
    let vals: Vec<String> = vec!["NEW".into(), "!CLOSED".into(), "OPEN".into()];
    let (pos, neg) = partition_filters(&vals);
    assert_eq!(pos, vec!["NEW", "OPEN"]);
    assert_eq!(neg, vec!["CLOSED"]);
}

#[test]
fn partition_filters_empty() {
    let vals: Vec<String> = vec![];
    let (pos, neg) = partition_filters(&vals);
    assert!(pos.is_empty());
    assert!(neg.is_empty());
}

#[test]
fn saved_query_list_roundtrips_json() {
    let query = SavedQuery {
        kind: QueryKind::List,
        product: vec!["Firefox".into()],
        component: vec![],
        status: vec!["NEW".into(), "ASSIGNED".into()],
        assignee: vec![],
        creator: vec![],
        priority: vec!["P1".into()],
        severity: vec![],
        quicksearch: None,
        limit: Some(25),
        fields: None,
        exclude_fields: None,
        source_url: None,
        server: None,
        raw_params: vec![],
        creation_time: None,
        last_change_time: None,
        whiteboard: vec![],
        target_milestone: vec![],
        version: vec![],
        op_sys: vec![],
        platform: vec![],
        resolution: vec![],
        qa_contact: vec![],
        url: vec![],
        order: None,
    };
    let json = serde_json::to_string(&query).unwrap();
    let roundtripped: SavedQuery = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtripped.kind, QueryKind::List);
    assert_eq!(roundtripped.product, vec!["Firefox"]);
    assert_eq!(roundtripped.status, vec!["NEW", "ASSIGNED"]);
    assert_eq!(roundtripped.limit, Some(25));
}

#[test]
fn saved_query_search_roundtrips_json() {
    let query = SavedQuery {
        kind: QueryKind::Search,
        quicksearch: Some("crash in tab".into()),
        limit: Some(10),
        ..SavedQuery::default()
    };
    let json = serde_json::to_string(&query).unwrap();
    let roundtripped: SavedQuery = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtripped.kind, QueryKind::Search);
    assert_eq!(roundtripped.quicksearch.as_deref(), Some("crash in tab"));
}

#[test]
fn saved_query_to_search_params_list() {
    let query = SavedQuery {
        kind: QueryKind::List,
        product: vec!["Core".into()],
        status: vec!["NEW".into()],
        limit: Some(20),
        fields: Some("id,summary".into()),
        ..SavedQuery::default()
    };
    let params = query.to_search_params();
    assert_eq!(params.product, vec!["Core"]);
    assert_eq!(params.status, vec!["NEW"]);
    assert_eq!(params.limit, Some(20));
    assert_eq!(params.include_fields.as_deref(), Some("id,summary"));
    assert!(params.quicksearch.is_none());
}

#[test]
fn saved_query_to_search_params_search() {
    let query = SavedQuery {
        kind: QueryKind::Search,
        quicksearch: Some("memory leak".into()),
        limit: Some(30),
        ..SavedQuery::default()
    };
    let params = query.to_search_params();
    assert_eq!(params.quicksearch.as_deref(), Some("memory leak"));
    assert_eq!(params.limit, Some(30));
    assert!(params.product.is_empty());
}

#[test]
fn saved_query_has_filters_false_empty() {
    let query = SavedQuery::default();
    assert!(!query.has_filters());
}

fn sample_raw_params() -> Vec<(String, String)> {
    vec![
        ("f1".into(), "qa_contact".into()),
        ("o1".into(), "changedfrom".into()),
    ]
}

#[test]
fn query_kind_url_serializes() {
    let json = serde_json::to_string(&QueryKind::Url).unwrap();
    assert_eq!(json, r#""url""#);
}

#[test]
fn query_kind_url_deserializes() {
    let kind: QueryKind = serde_json::from_str(r#""url""#).unwrap();
    assert_eq!(kind, QueryKind::Url);
}

#[test]
fn saved_query_with_url_fields_roundtrips() {
    let query = SavedQuery {
        kind: QueryKind::Url,
        source_url: Some("https://bugzilla.example.com/buglist.cgi?product=Firefox".into()),
        server: Some("example".into()),
        raw_params: vec![
            ("f1".into(), "qa_contact".into()),
            ("o1".into(), "changedfrom".into()),
            ("v1".into(), "user@example.com".into()),
        ],
        product: vec!["Firefox".into()],
        ..SavedQuery::default()
    };
    let json = serde_json::to_string(&query).unwrap();
    let roundtripped: SavedQuery = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtripped.kind, QueryKind::Url);
    assert_eq!(
        roundtripped.source_url.as_deref(),
        Some("https://bugzilla.example.com/buglist.cgi?product=Firefox")
    );
    assert_eq!(roundtripped.server.as_deref(), Some("example"));
    assert_eq!(roundtripped.raw_params.len(), 3);
    assert_eq!(
        roundtripped.raw_params[0],
        ("f1".into(), "qa_contact".into())
    );
    assert_eq!(roundtripped.product, vec!["Firefox"]);
}

#[test]
fn saved_query_without_url_fields_omits_them_in_json() {
    let query = SavedQuery {
        kind: QueryKind::List,
        product: vec!["Firefox".into()],
        ..SavedQuery::default()
    };
    let json = serde_json::to_string(&query).unwrap();
    assert!(!json.contains("source_url"));
    assert!(!json.contains("\"server\""));
    assert!(!json.contains("raw_params"));
}

#[test]
fn saved_query_url_kind_to_search_params_includes_raw_params() {
    let query = SavedQuery {
        kind: QueryKind::Url,
        product: vec!["Firefox".into()],
        raw_params: sample_raw_params(),
        limit: Some(100),
        ..SavedQuery::default()
    };
    let params = query.to_search_params();
    assert_eq!(params.product, vec!["Firefox"]);
    assert_eq!(params.limit, Some(100));
    assert_eq!(params.raw_params.len(), 2);
    assert_eq!(params.raw_params[0], ("f1".into(), "qa_contact".into()));
}

#[test]
fn field_mappings_covers_all_search_params_vec_fields() {
    let params = SearchParams::default();
    for mapping in FIELD_MAPPINGS {
        let field = params.get_field(mapping.field);
        assert!(
            field.is_empty(),
            "default field should be empty: {}",
            mapping.struct_field
        );
    }
}

#[test]
fn field_mappings_has_expected_count() {
    assert_eq!(FIELD_MAPPINGS.len(), 15);
}

#[test]
fn field_mappings_negation_operators_match_field_kind() {
    let by_struct = |name: &str| {
        FIELD_MAPPINGS
            .iter()
            .find(|m| m.struct_field == name)
            .unwrap_or_else(|| panic!("missing field mapping: {name}"))
    };
    // Substring fields use NotSubstring.
    assert_eq!(
        by_struct("whiteboard").negation_operator,
        NegationOp::NotSubstring
    );
    assert_eq!(by_struct("url").negation_operator, NegationOp::NotSubstring);
    // Exact-match fields use NotEquals.
    for f in [
        "product",
        "component",
        "status",
        "assigned_to",
        "creator",
        "priority",
        "severity",
        "target_milestone",
        "version",
        "op_sys",
        "platform",
        "resolution",
        "qa_contact",
    ] {
        assert_eq!(
            by_struct(f).negation_operator,
            NegationOp::NotEquals,
            "field {f} should use NotEquals"
        );
    }
}

#[test]
fn negation_op_as_str_matches_bugzilla_wire_form() {
    assert_eq!(NegationOp::NotEquals.as_str(), "notequals");
    assert_eq!(NegationOp::NotSubstring.as_str(), "notsubstring");
}

#[test]
fn field_mappings_url_param_lookup() {
    let status = FIELD_MAPPINGS.iter().find(|m| m.url_param == "bug_status");
    assert!(status.is_some());
    assert_eq!(status.unwrap().struct_field, "status");
    assert_eq!(status.unwrap().internal_name, "bug_status");
}

#[test]
fn field_mappings_internal_name_for_creator() {
    let creator = FIELD_MAPPINGS.iter().find(|m| m.struct_field == "creator");
    assert!(creator.is_some());
    assert_eq!(creator.unwrap().internal_name, "reporter");
}

#[test]
fn search_params_get_field_returns_correct_data() {
    let params = SearchParams {
        product: vec!["Firefox".into()],
        status: vec!["NEW".into(), "ASSIGNED".into()],
        ..Default::default()
    };
    assert_eq!(
        params.get_field(FilterField::Product),
        ["Firefox".to_string()]
    );
    assert_eq!(
        params.get_field(FilterField::Status),
        ["NEW".to_string(), "ASSIGNED".to_string()]
    );
    assert!(params.get_field(FilterField::Creator).is_empty());
}

#[test]
fn search_params_get_field_mut_updates_every_mapped_field() {
    let mut params = SearchParams::default();

    for mapping in FIELD_MAPPINGS {
        params
            .get_field_mut(mapping.field)
            .push(format!("value-{}", mapping.struct_field));
    }

    for mapping in FIELD_MAPPINGS {
        assert_eq!(
            params.get_field(mapping.field),
            [format!("value-{}", mapping.struct_field)],
            "mapped field should roundtrip through mutable and immutable access: {}",
            mapping.struct_field
        );
    }
}

#[test]
fn saved_query_get_field_mut_returns_correct_fields() {
    let mut query = SavedQuery::default();
    query
        .get_field_mut(FilterField::AssignedTo)
        .push("dev@example.com".into());
    assert_eq!(query.assignee, vec!["dev@example.com"]);

    query.get_field_mut(FilterField::Status).push("NEW".into());
    assert_eq!(query.status, vec!["NEW"]);
}

#[test]
fn search_params_has_filters_for_each_individual_field() {
    type Setter = fn(&mut SearchParams);
    let cases: &[(&str, Setter)] = &[
        ("product", |p| p.product.push("X".into())),
        ("component", |p| p.component.push("X".into())),
        ("status", |p| p.status.push("X".into())),
        ("assigned_to", |p| p.assigned_to.push("X".into())),
        ("creator", |p| p.creator.push("X".into())),
        ("priority", |p| p.priority.push("X".into())),
        ("severity", |p| p.severity.push("X".into())),
        ("cc", |p| p.cc = Some("X".into())),
        ("alias", |p| p.alias = Some("X".into())),
        ("id", |p| p.id = vec![1]),
        ("summary", |p| p.summary = Some("X".into())),
        ("quicksearch", |p| p.quicksearch = Some("X".into())),
        ("raw_params", |p| {
            p.raw_params = vec![("f1".into(), "X".into())];
        }),
        ("creation_time", |p| {
            p.creation_time = Some("2026-04-01T00:00:00Z".into());
        }),
        ("last_change_time", |p| {
            p.last_change_time = Some("2026-04-01T00:00:00Z".into());
        }),
        ("whiteboard", |p| p.whiteboard.push("X".into())),
        ("target_milestone", |p| p.target_milestone.push("X".into())),
        ("version", |p| p.version.push("X".into())),
        ("op_sys", |p| p.op_sys.push("X".into())),
        ("platform", |p| p.platform.push("X".into())),
        ("resolution", |p| p.resolution.push("X".into())),
        ("qa_contact", |p| p.qa_contact.push("X".into())),
        ("url", |p| p.url.push("X".into())),
    ];
    for (name, setter) in cases {
        let mut p = SearchParams::default();
        setter(&mut p);
        assert!(
            p.has_filters(),
            "field `{name}` alone should make has_filters() return true"
        );
    }
}

#[test]
fn search_params_has_structured_filters_excludes_freetext() {
    // Free-text fields (quicksearch, summary) must NOT count as structured
    // filters: an empty REST result for these is authoritative across
    // transports, so the hybrid-mode XML-RPC fallback must not fire.
    let p = SearchParams {
        quicksearch: Some("anything".into()),
        ..Default::default()
    };
    assert!(!p.has_structured_filters());

    let p = SearchParams {
        summary: Some("anything".into()),
        ..Default::default()
    };
    assert!(!p.has_structured_filters());

    let p = SearchParams {
        quicksearch: Some("a".into()),
        summary: Some("b".into()),
        ..Default::default()
    };
    assert!(!p.has_structured_filters());
}

#[test]
fn search_params_has_structured_filters_for_each_individual_field() {
    type Setter = fn(&mut SearchParams);
    let cases: &[(&str, Setter)] = &[
        ("product", |p| p.product.push("X".into())),
        ("component", |p| p.component.push("X".into())),
        ("status", |p| p.status.push("X".into())),
        ("assigned_to", |p| p.assigned_to.push("X".into())),
        ("creator", |p| p.creator.push("X".into())),
        ("priority", |p| p.priority.push("X".into())),
        ("severity", |p| p.severity.push("X".into())),
        ("cc", |p| p.cc = Some("X".into())),
        ("alias", |p| p.alias = Some("X".into())),
        ("id", |p| p.id = vec![1]),
        ("raw_params", |p| {
            p.raw_params = vec![("f1".into(), "X".into())];
        }),
        ("creation_time", |p| {
            p.creation_time = Some("2026-04-01T00:00:00Z".into());
        }),
        ("last_change_time", |p| {
            p.last_change_time = Some("2026-04-01T00:00:00Z".into());
        }),
        ("whiteboard", |p| p.whiteboard.push("X".into())),
        ("target_milestone", |p| p.target_milestone.push("X".into())),
        ("version", |p| p.version.push("X".into())),
        ("op_sys", |p| p.op_sys.push("X".into())),
        ("platform", |p| p.platform.push("X".into())),
        ("resolution", |p| p.resolution.push("X".into())),
        ("qa_contact", |p| p.qa_contact.push("X".into())),
        ("url", |p| p.url.push("X".into())),
    ];
    for (name, setter) in cases {
        let mut p = SearchParams::default();
        setter(&mut p);
        assert!(
            p.has_structured_filters(),
            "field `{name}` alone should make has_structured_filters() return true"
        );
    }
}

#[test]
fn search_params_has_structured_filters_empty() {
    assert!(!SearchParams::default().has_structured_filters());
}

#[test]
fn saved_query_has_filters_for_each_individual_field() {
    type Setter = fn(&mut SavedQuery);
    let cases: &[(&str, Setter)] = &[
        ("product", |q| q.product.push("X".into())),
        ("component", |q| q.component.push("X".into())),
        ("status", |q| q.status.push("X".into())),
        ("assignee", |q| q.assignee.push("X".into())),
        ("creator", |q| q.creator.push("X".into())),
        ("priority", |q| q.priority.push("X".into())),
        ("severity", |q| q.severity.push("X".into())),
        ("quicksearch", |q| q.quicksearch = Some("X".into())),
        ("raw_params", |q| {
            q.raw_params = vec![("f1".into(), "X".into())];
        }),
        ("creation_time", |q: &mut SavedQuery| {
            q.creation_time = Some("2026-04-01T00:00:00Z".into());
        }),
        ("last_change_time", |q: &mut SavedQuery| {
            q.last_change_time = Some("2026-04-01T00:00:00Z".into());
        }),
        ("whiteboard", |q: &mut SavedQuery| {
            q.whiteboard.push("X".into());
        }),
        ("target_milestone", |q: &mut SavedQuery| {
            q.target_milestone.push("X".into());
        }),
        ("version", |q: &mut SavedQuery| q.version.push("X".into())),
        ("op_sys", |q: &mut SavedQuery| q.op_sys.push("X".into())),
        ("platform", |q: &mut SavedQuery| q.platform.push("X".into())),
        ("resolution", |q: &mut SavedQuery| {
            q.resolution.push("X".into());
        }),
        ("qa_contact", |q: &mut SavedQuery| {
            q.qa_contact.push("X".into());
        }),
        ("url", |q: &mut SavedQuery| q.url.push("X".into())),
    ];
    for (name, setter) in cases {
        let mut q = SavedQuery::default();
        setter(&mut q);
        assert!(
            q.has_filters(),
            "field `{name}` alone should make has_filters() return true"
        );
    }
}

#[test]
fn id_list_update_is_empty_when_both_empty() {
    let upd = IdListUpdate {
        add: vec![],
        remove: vec![],
    };
    assert!(upd.is_empty());
}

#[test]
fn id_list_update_not_empty_when_only_add() {
    let upd = IdListUpdate {
        add: vec![1],
        remove: vec![],
    };
    assert!(!upd.is_empty());
}

#[test]
fn id_list_update_not_empty_when_only_remove() {
    let upd = IdListUpdate {
        add: vec![],
        remove: vec![2],
    };
    assert!(!upd.is_empty());
}

#[test]
fn into_search_params_moves_fields() {
    let query = SavedQuery {
        kind: QueryKind::List,
        product: vec!["Firefox".into()],
        component: vec!["General".into()],
        status: vec!["NEW".into()],
        assignee: vec!["dev@example.com".into()],
        creator: vec!["reporter@example.com".into()],
        priority: vec!["P1".into()],
        severity: vec!["critical".into()],
        quicksearch: Some("crash".into()),
        limit: Some(25),
        fields: Some("id,summary".into()),
        exclude_fields: Some("comments".into()),
        raw_params: vec![("f1".into(), "qa_contact".into())],
        ..Default::default()
    };
    let params = query.into_search_params();
    assert_eq!(params.product, vec!["Firefox"]);
    assert_eq!(params.component, vec!["General"]);
    assert_eq!(params.status, vec!["NEW"]);
    assert_eq!(params.assigned_to, vec!["dev@example.com"]);
    assert_eq!(params.creator, vec!["reporter@example.com"]);
    assert_eq!(params.priority, vec!["P1"]);
    assert_eq!(params.severity, vec!["critical"]);
    assert_eq!(params.quicksearch, Some("crash".into()));
    assert_eq!(params.limit, Some(25));
    assert_eq!(params.include_fields, Some("id,summary".into()));
    assert_eq!(params.exclude_fields, Some("comments".into()));
    assert_eq!(params.raw_params, vec![("f1".into(), "qa_contact".into())]);
}

#[test]
fn saved_query_into_search_params_forwards_date_filters() {
    let q = SavedQuery {
        creation_time: Some("2026-04-01T00:00:00Z".into()),
        last_change_time: Some("2026-04-15T00:00:00Z".into()),
        ..SavedQuery::default()
    };
    let p = q.into_search_params();
    assert_eq!(p.creation_time.as_deref(), Some("2026-04-01T00:00:00Z"));
    assert_eq!(p.last_change_time.as_deref(), Some("2026-04-15T00:00:00Z"));
}

#[test]
fn saved_query_toml_roundtrip_preserves_date_filters() {
    let q = SavedQuery {
        product: vec!["Firefox".into()],
        creation_time: Some("2026-04-01T00:00:00Z".into()),
        last_change_time: Some("2026-04-15T00:00:00Z".into()),
        ..SavedQuery::default()
    };
    let toml_str = toml::to_string(&q).unwrap();
    let parsed: SavedQuery = toml::from_str(&toml_str).unwrap();
    assert_eq!(
        parsed.creation_time.as_deref(),
        Some("2026-04-01T00:00:00Z")
    );
    assert_eq!(
        parsed.last_change_time.as_deref(),
        Some("2026-04-15T00:00:00Z")
    );
}

#[test]
fn saved_query_toml_legacy_without_date_filters_deserializes() {
    // Configs written before this change must still parse — both fields
    // are #[serde(default)] so missing keys produce None.
    let toml_str = r#"
product = ["Firefox"]
"#;
    let parsed: SavedQuery = toml::from_str(toml_str).unwrap();
    assert_eq!(parsed.creation_time, None);
    assert_eq!(parsed.last_change_time, None);
}

#[test]
fn apply_overrides_replaces_date_filters_when_some() {
    let mut p = SearchParams {
        creation_time: Some("2026-04-01T00:00:00Z".into()),
        last_change_time: Some("2026-04-15T00:00:00Z".into()),
        ..Default::default()
    };
    p.apply_overrides(Overrides {
        creation_time: Some("2026-05-01T00:00:00Z"),
        ..Default::default()
    });
    assert_eq!(p.creation_time.as_deref(), Some("2026-05-01T00:00:00Z"));
    // last_change_time unchanged because we passed None.
    assert_eq!(p.last_change_time.as_deref(), Some("2026-04-15T00:00:00Z"));
}

#[test]
fn apply_overrides_keeps_date_filters_when_none() {
    let mut p = SearchParams {
        creation_time: Some("2026-04-01T00:00:00Z".into()),
        ..Default::default()
    };
    p.apply_overrides(Overrides {
        limit: Some(10),
        ..Default::default()
    });
    assert_eq!(p.creation_time.as_deref(), Some("2026-04-01T00:00:00Z"));
}

#[test]
fn saved_query_into_search_params_forwards_158_fields() {
    let q = SavedQuery {
        whiteboard: vec!["needs-review".into()],
        target_milestone: vec!["5.0".into()],
        version: vec!["9.4".into()],
        op_sys: vec!["Linux".into()],
        platform: vec!["x86_64".into()],
        resolution: vec!["FIXED".into()],
        qa_contact: vec!["qa@example.com".into()],
        url: vec!["github.com/foo".into()],
        ..SavedQuery::default()
    };
    let p = q.into_search_params();
    assert_eq!(p.whiteboard, vec!["needs-review"]);
    assert_eq!(p.target_milestone, vec!["5.0"]);
    assert_eq!(p.version, vec!["9.4"]);
    assert_eq!(p.op_sys, vec!["Linux"]);
    assert_eq!(p.platform, vec!["x86_64"]);
    assert_eq!(p.resolution, vec!["FIXED"]);
    assert_eq!(p.qa_contact, vec!["qa@example.com"]);
    assert_eq!(p.url, vec!["github.com/foo"]);
}

#[test]
fn saved_query_toml_roundtrip_preserves_158_fields() {
    let q = SavedQuery {
        whiteboard: vec!["needs-review".into()],
        target_milestone: vec!["5.0".into()],
        version: vec!["9.4".into()],
        op_sys: vec!["Linux".into()],
        platform: vec!["x86_64".into()],
        resolution: vec!["FIXED".into()],
        qa_contact: vec!["qa@example.com".into()],
        url: vec!["github.com/foo".into()],
        ..SavedQuery::default()
    };
    let toml_str = toml::to_string(&q).unwrap();
    let parsed: SavedQuery = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.whiteboard, vec!["needs-review"]);
    assert_eq!(parsed.target_milestone, vec!["5.0"]);
    assert_eq!(parsed.version, vec!["9.4"]);
    assert_eq!(parsed.op_sys, vec!["Linux"]);
    assert_eq!(parsed.platform, vec!["x86_64"]);
    assert_eq!(parsed.resolution, vec!["FIXED"]);
    assert_eq!(parsed.qa_contact, vec!["qa@example.com"]);
    assert_eq!(parsed.url, vec!["github.com/foo"]);
}

#[test]
fn saved_query_toml_legacy_without_158_fields_deserializes() {
    // Configs written before #158 must still parse — every new
    // field is `#[serde(default)]` so missing keys produce empty
    // Vecs.
    let toml_str = r#"
product = ["Firefox"]
"#;
    let parsed: SavedQuery = toml::from_str(toml_str).unwrap();
    assert!(parsed.whiteboard.is_empty());
    assert!(parsed.url.is_empty());
}

#[test]
fn apply_overrides_replaces_158_fields_when_some() {
    let mut p = SearchParams {
        whiteboard: vec!["original".into()],
        resolution: vec!["FIXED".into()],
        ..Default::default()
    };
    let new_wb: Vec<String> = vec!["overridden".into()];
    let new_res: Vec<String> = vec!["WONTFIX".into()];
    p.apply_overrides(Overrides {
        whiteboard: Some(&new_wb),
        resolution: Some(&new_res),
        ..Default::default()
    });
    assert_eq!(p.whiteboard, vec!["overridden"]);
    assert_eq!(p.resolution, vec!["WONTFIX"]);
}

#[test]
fn apply_overrides_keeps_158_fields_when_none() {
    let mut p = SearchParams {
        whiteboard: vec!["original".into()],
        ..Default::default()
    };
    p.apply_overrides(Overrides::default());
    assert_eq!(p.whiteboard, vec!["original"]);
}

#[test]
fn apply_overrides_default_is_noop() {
    let mut p = SearchParams {
        product: vec!["P".into()],
        whiteboard: vec!["wip".into()],
        creation_time: Some("2026-04-01T00:00:00Z".into()),
        ..Default::default()
    };
    p.apply_overrides(Overrides::default());
    assert_eq!(p.product, vec!["P"]);
    assert_eq!(p.whiteboard, vec!["wip"]);
    assert_eq!(p.creation_time.as_deref(), Some("2026-04-01T00:00:00Z"));
}

#[test]
fn string_list_update_is_empty_when_both_empty() {
    let upd = StringListUpdate {
        add: vec![],
        remove: vec![],
    };
    assert!(upd.is_empty());
}

#[test]
fn string_list_update_not_empty_when_only_add() {
    let upd = StringListUpdate {
        add: vec!["fix-needed".to_string()],
        remove: vec![],
    };
    assert!(!upd.is_empty());
}

#[test]
fn string_list_update_not_empty_when_only_remove() {
    let upd = StringListUpdate {
        add: vec![],
        remove: vec!["regression".to_string()],
    };
    assert!(!upd.is_empty());
}

#[test]
fn string_list_update_serializes_with_add_and_remove() {
    let upd = StringListUpdate {
        add: vec!["a".to_string(), "b".to_string()],
        remove: vec!["c".to_string()],
    };
    let json = serde_json::to_value(&upd).unwrap();
    assert_eq!(
        json,
        serde_json::json!({"add": ["a", "b"], "remove": ["c"]})
    );
}

#[test]
fn string_list_update_skips_empty_add() {
    let upd = StringListUpdate {
        add: vec![],
        remove: vec!["c".to_string()],
    };
    let json = serde_json::to_value(&upd).unwrap();
    assert_eq!(json, serde_json::json!({"remove": ["c"]}));
}

#[test]
fn string_list_update_skips_empty_remove() {
    let upd = StringListUpdate {
        add: vec!["a".to_string()],
        remove: vec![],
    };
    let json = serde_json::to_value(&upd).unwrap();
    assert_eq!(json, serde_json::json!({"add": ["a"]}));
}

#[test]
fn update_bug_params_omits_empty_string_lists() {
    let params = UpdateBugParams::default();
    let json = serde_json::to_value(&params).unwrap();
    assert!(json.get("keywords").is_none());
    assert!(json.get("cc").is_none());
    assert!(json.get("groups").is_none());
    assert!(json.get("see_also").is_none());
}

#[test]
fn bug_deserializes_dupe_of() {
    let bug: Bug = serde_json::from_value(serde_json::json!({
        "id": 101,
        "summary": "duplicate source",
        "status": "RESOLVED",
        "resolution": "DUPLICATE",
        "dupe_of": 202
    }))
    .unwrap();

    assert_eq!(bug.dupe_of, Some(202));
}

#[test]
fn bug_defaults_missing_dupe_of_to_none() {
    let bug: Bug = serde_json::from_value(serde_json::json!({
        "id": 101,
        "summary": "ordinary bug",
        "status": "NEW"
    }))
    .unwrap();

    assert_eq!(bug.dupe_of, None);
}

#[test]
fn update_bug_params_serializes_dupe_of() {
    let params = UpdateBugParams {
        dupe_of: Some(202),
        ..Default::default()
    };
    let json = serde_json::to_value(&params).unwrap();

    assert_eq!(json, serde_json::json!({"dupe_of": 202}));
}

#[test]
fn update_bug_params_omits_dupe_of_when_none() {
    let params = UpdateBugParams::default();
    let json = serde_json::to_value(&params).unwrap();

    assert!(json.get("dupe_of").is_none());
}

#[test]
fn update_bug_params_serializes_scalar_parity_fields() {
    let params = UpdateBugParams {
        alias: Some("short-name".into()),
        deadline: Some("2026-12-31".into()),
        estimated_time: Some(3.5),
        remaining_time: Some(1.25),
        work_time: Some(0.5),
        url: Some("https://example.com/repro".into()),
        target_milestone: Some("5.0".into()),
        ..Default::default()
    };
    let json = serde_json::to_value(&params).unwrap();

    assert_eq!(json["alias"], "short-name");
    assert_eq!(json["deadline"], "2026-12-31");
    assert_eq!(json["estimated_time"], 3.5);
    assert_eq!(json["remaining_time"], 1.25);
    assert_eq!(json["work_time"], 0.5);
    assert_eq!(json["url"], "https://example.com/repro");
    assert_eq!(json["target_milestone"], "5.0");
}

#[test]
fn update_bug_params_serializes_reset_flags_only_when_true() {
    let params = UpdateBugParams {
        reset_assigned_to: true,
        reset_qa_contact: true,
        ..Default::default()
    };
    let json = serde_json::to_value(&params).unwrap();

    assert_eq!(json["reset_assigned_to"], true);
    assert_eq!(json["reset_qa_contact"], true);
}

#[test]
fn update_bug_params_default_omits_scalar_parity_fields() {
    let params = UpdateBugParams::default();
    let json = serde_json::to_value(&params).unwrap();

    for key in [
        "alias",
        "deadline",
        "estimated_time",
        "remaining_time",
        "work_time",
        "url",
        "target_milestone",
        "reset_assigned_to",
        "reset_qa_contact",
    ] {
        assert!(
            json.get(key).is_none(),
            "expected {key} to be omitted: {json}"
        );
    }
}

#[test]
fn update_bug_params_serializes_string_lists() {
    let params = UpdateBugParams {
        keywords: StringListUpdate {
            add: vec!["fix-needed".to_string()],
            remove: vec!["wontfix".to_string()],
        },
        cc: StringListUpdate {
            add: vec!["alice@example.com".to_string()],
            remove: vec![],
        },
        groups: StringListUpdate {
            add: vec![],
            remove: vec!["secret".to_string()],
        },
        see_also: StringListUpdate {
            add: vec!["https://example.com/issue/1".to_string()],
            remove: vec![],
        },
        ..Default::default()
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(
        json["keywords"],
        serde_json::json!({"add": ["fix-needed"], "remove": ["wontfix"]})
    );
    assert_eq!(
        json["cc"],
        serde_json::json!({"add": ["alice@example.com"]})
    );
    assert_eq!(json["groups"], serde_json::json!({"remove": ["secret"]}));
    assert_eq!(
        json["see_also"],
        serde_json::json!({"add": ["https://example.com/issue/1"]})
    );
}

#[test]
fn comment_update_serializes_public_body() {
    let upd = CommentUpdate {
        body: "hi".into(),
        is_private: false,
    };
    let json = serde_json::to_value(&upd).unwrap();
    assert_eq!(json, serde_json::json!({"body": "hi"}));
}

#[test]
fn comment_update_serializes_private_body() {
    let upd = CommentUpdate {
        body: "hi".into(),
        is_private: true,
    };
    let json = serde_json::to_value(&upd).unwrap();
    assert_eq!(json, serde_json::json!({"body": "hi", "is_private": true}));
}

#[test]
fn update_bug_params_omits_comment_when_none() {
    let params = UpdateBugParams::default();
    let json = serde_json::to_value(&params).unwrap();
    assert!(
        json.get("comment").is_none(),
        "expected no comment key when None, got: {json}"
    );
}

#[test]
fn update_bug_params_serializes_comment_when_some() {
    let params = UpdateBugParams {
        summary: Some("new summary".into()),
        comment: Some(CommentUpdate {
            body: "see #other".into(),
            is_private: false,
        }),
        ..Default::default()
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["summary"], "new summary");
    assert_eq!(json["comment"], serde_json::json!({"body": "see #other"}));
}

#[test]
fn update_bug_params_serializes_private_comment() {
    let params = UpdateBugParams {
        comment: Some(CommentUpdate {
            body: "secret".into(),
            is_private: true,
        }),
        ..Default::default()
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(
        json["comment"],
        serde_json::json!({"body": "secret", "is_private": true})
    );
}

#[test]
fn update_bug_params_default_omits_comment_is_private() {
    let params = UpdateBugParams::default();
    let json = serde_json::to_value(&params).unwrap();
    assert!(
        !json.as_object().unwrap().contains_key("comment_is_private"),
        "empty comment_is_private map should be skipped on the wire, got {json}"
    );
}

#[test]
fn update_bug_params_serializes_comment_is_private_map() {
    use std::collections::HashMap;
    let mut map = HashMap::new();
    map.insert(5678u64, true);
    let params = UpdateBugParams {
        comment_is_private: map,
        ..Default::default()
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(
        json["comment_is_private"]["5678"],
        serde_json::Value::Bool(true)
    );
}

/// Minimal create params (only the required fields) must not emit any of the
/// optional parity fields, so the server applies its own defaults.
fn minimal_create_params() -> CreateBugParams {
    CreateBugParams {
        product: "Prod".into(),
        component: "Comp".into(),
        summary: "Sum".into(),
        version: "1.0".into(),
        ..Default::default()
    }
}

#[test]
fn create_params_omit_unset_parity_fields() {
    let json = serde_json::to_value(minimal_create_params()).unwrap();
    let obj = json.as_object().unwrap();
    for key in [
        "alias",
        "url",
        "whiteboard",
        "target_milestone",
        "deadline",
        "cc",
        "keywords",
        "groups",
        "flags",
    ] {
        assert!(
            !obj.contains_key(key),
            "unset field '{key}' must be omitted"
        );
    }
}

#[test]
fn create_params_serialize_parity_fields() {
    let params = CreateBugParams {
        alias: Some("my-alias".into()),
        url: Some("https://example.com/repro".into()),
        whiteboard: Some("needs-triage".into()),
        target_milestone: Some("M1".into()),
        deadline: Some("2026-12-31".into()),
        cc: vec!["a@example.com".into(), "b@example.com".into()],
        keywords: vec!["regression".into()],
        groups: vec!["security".into()],
        flags: vec![FlagUpdate {
            name: "review".into(),
            status: crate::types::FlagStatus::Grant,
            requestee: None,
        }],
        ..minimal_create_params()
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["alias"], "my-alias");
    assert_eq!(json["url"], "https://example.com/repro");
    assert_eq!(json["whiteboard"], "needs-triage");
    assert_eq!(json["target_milestone"], "M1");
    assert_eq!(json["deadline"], "2026-12-31");
    assert_eq!(json["cc"][0], "a@example.com");
    assert_eq!(json["cc"][1], "b@example.com");
    assert_eq!(json["keywords"][0], "regression");
    assert_eq!(json["groups"][0], "security");
    // Flags serialize as the Bug.create array shape: {name, status}.
    assert_eq!(json["flags"][0]["name"], "review");
    assert_eq!(json["flags"][0]["status"], "+");
}
