#![expect(clippy::unwrap_used)]

use super::{QueryKind, SavedQuery};
use crate::types::bug::FilterField;

#[test]
fn saved_query_list_roundtrips_json() {
    let query = SavedQuery {
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
    assert_eq!(roundtripped.kind(), QueryKind::List);
    assert_eq!(roundtripped.product, vec!["Firefox"]);
    assert_eq!(roundtripped.status, vec!["NEW", "ASSIGNED"]);
    assert_eq!(roundtripped.limit, Some(25));
}

#[test]
fn saved_query_search_roundtrips_json() {
    let query = SavedQuery {
        quicksearch: Some("crash in tab".into()),
        limit: Some(10),
        ..SavedQuery::default()
    };
    let json = serde_json::to_string(&query).unwrap();
    let roundtripped: SavedQuery = serde_json::from_str(&json).unwrap();
    assert_eq!(roundtripped.kind(), QueryKind::Search);
    assert_eq!(roundtripped.quicksearch.as_deref(), Some("crash in tab"));
}

#[test]
fn saved_query_kind_is_derived_from_executable_fields() {
    let stale_json = r#"{"kind":"list","quicksearch":"crash in tab"}"#;
    let query: SavedQuery = serde_json::from_str(stale_json).unwrap();

    assert_eq!(query.kind(), QueryKind::Search);

    let serialized: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&query).unwrap()).unwrap();
    assert_eq!(serialized["kind"], "search");
}

#[test]
fn saved_query_to_search_params_list() {
    let query = SavedQuery {
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
    assert_eq!(roundtripped.kind(), QueryKind::Url);
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
fn into_search_params_moves_fields() {
    let query = SavedQuery {
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
    let toml_str = r#"
product = ["Firefox"]
"#;
    let parsed: SavedQuery = toml::from_str(toml_str).unwrap();
    assert_eq!(parsed.creation_time, None);
    assert_eq!(parsed.last_change_time, None);
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
    let toml_str = r#"
product = ["Firefox"]
"#;
    let parsed: SavedQuery = toml::from_str(toml_str).unwrap();
    assert!(parsed.whiteboard.is_empty());
    assert!(parsed.url.is_empty());
}
