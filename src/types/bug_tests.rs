#![expect(clippy::unwrap_used)]

use super::*;

#[test]
fn bug_deserializes_minimal() {
    let json = r#"{"id": 42}"#;
    let bug: Bug = serde_json::from_str(json).unwrap();
    assert_eq!(bug.id, 42);
    assert!(bug.summary.is_empty());
    assert!(bug.keywords.is_empty());
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
fn field_value_null_name_becomes_empty() {
    let json = r#"{"name": null, "sort_key": 0, "is_active": true}"#;
    let fv: FieldValue = serde_json::from_str(json).unwrap();
    assert!(fv.name.is_empty());
}

#[test]
fn field_value_with_name() {
    let json = r#"{"name": "RESOLVED", "sort_key": 5, "is_active": true}"#;
    let fv: FieldValue = serde_json::from_str(json).unwrap();
    assert_eq!(fv.name, "RESOLVED");
    assert_eq!(fv.sort_key, 5);
    assert!(fv.is_active);
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
        let field = params.get_field(mapping.struct_field);
        assert!(
            field.is_empty(),
            "default field should be empty: {}",
            mapping.struct_field
        );
    }
}

#[test]
fn field_mappings_has_expected_count() {
    assert_eq!(FIELD_MAPPINGS.len(), 7);
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
    assert_eq!(params.get_field("product"), &["Firefox"]);
    assert_eq!(params.get_field("status"), &["NEW", "ASSIGNED"]);
    assert!(params.get_field("creator").is_empty());
}

#[test]
#[should_panic(expected = "unknown field")]
fn search_params_get_field_panics_on_unknown() {
    let params = SearchParams::default();
    params.get_field("nonexistent");
}

#[test]
fn saved_query_get_field_mut_returns_correct_fields() {
    let mut query = SavedQuery::default();
    query
        .get_field_mut("assigned_to")
        .unwrap()
        .push("dev@example.com".into());
    assert_eq!(query.assignee, vec!["dev@example.com"]);

    query.get_field_mut("status").unwrap().push("NEW".into());
    assert_eq!(query.status, vec!["NEW"]);
}

#[test]
fn saved_query_get_field_mut_returns_none_for_unknown() {
    let mut query = SavedQuery::default();
    assert!(query.get_field_mut("nonexistent").is_none());
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
            p.creation_time = Some("2026-04-01T00:00:00Z".into())
        }),
        ("last_change_time", |p| {
            p.last_change_time = Some("2026-04-01T00:00:00Z".into())
        }),
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
            p.creation_time = Some("2026-04-01T00:00:00Z".into())
        }),
        ("last_change_time", |p| {
            p.last_change_time = Some("2026-04-01T00:00:00Z".into())
        }),
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
