#![expect(clippy::unwrap_used)]

use crate::types::bug::{BugLink, BugLinksNode, LinkRelation};

#[test]
fn relation_all_is_fixed_order_with_wire_names() {
    let names: Vec<&str> = LinkRelation::ALL.iter().map(|r| r.as_str()).collect();
    assert_eq!(
        names,
        [
            "depends_on",
            "blocks",
            "dupe_of",
            "duplicates",
            "regressed_by",
            "regressions"
        ]
    );
}

#[test]
fn direction_mapping_matches_spec() {
    for r in [
        LinkRelation::DependsOn,
        LinkRelation::DupeOf,
        LinkRelation::RegressedBy,
    ] {
        assert_eq!(
            r.direction().as_str(),
            "out",
            "{} should be out",
            r.as_str()
        );
    }
    for r in [
        LinkRelation::Blocks,
        LinkRelation::Duplicates,
        LinkRelation::Regressions,
    ] {
        assert_eq!(r.direction().as_str(), "in", "{} should be in", r.as_str());
    }
}

#[test]
fn from_str_accepts_wire_names_and_rejects_garbage() {
    assert_eq!(
        "depends_on".parse::<LinkRelation>().unwrap(),
        LinkRelation::DependsOn
    );
    assert_eq!(
        "regressions".parse::<LinkRelation>().unwrap(),
        LinkRelation::Regressions
    );
    let err = "bogus".parse::<LinkRelation>().unwrap_err();
    assert!(
        err.contains("depends_on"),
        "error names valid values: {err}"
    );
}

#[test]
fn edges_are_fixed_relation_order_then_ascending_id() {
    let node = BugLinksNode {
        id: 1,
        summary: None,
        status: None,
        depends_on: vec![30, 10],
        blocks: vec![20],
        dupe_of: Some(5),
        duplicates: vec![],
        regressed_by: vec![],
        regressions: vec![],
    };
    let edges = node.edges(None);
    assert_eq!(
        edges,
        vec![
            (LinkRelation::DependsOn, 10),
            (LinkRelation::DependsOn, 30),
            (LinkRelation::Blocks, 20),
            (LinkRelation::DupeOf, 5),
        ]
    );
}

#[test]
fn edges_filter_restricts_to_one_relation() {
    let node = BugLinksNode {
        id: 1,
        summary: None,
        status: None,
        depends_on: vec![10],
        blocks: vec![20],
        dupe_of: None,
        duplicates: vec![],
        regressed_by: vec![],
        regressions: vec![],
    };
    assert_eq!(
        node.edges(Some(LinkRelation::Blocks)),
        vec![(LinkRelation::Blocks, 20)]
    );
}

#[test]
fn node_defaults_missing_fields() {
    let node: BugLinksNode = serde_json::from_str(r#"{"id":7}"#).unwrap();
    assert_eq!(node.id, 7);
    assert!(node.depends_on.is_empty() && node.duplicates.is_empty());
    assert_eq!(node.dupe_of, None);
}

#[test]
fn bug_link_serializes_in_documented_key_order() {
    let link = BugLink {
        id: 12346,
        relation: LinkRelation::DependsOn,
        direction: LinkRelation::DependsOn.direction(),
        depth: 1,
        summary: Some("s".into()),
        status: Some("NEW".into()),
    };
    let v = serde_json::to_string(&link).unwrap();
    assert_eq!(
        v,
        r#"{"id":12346,"relation":"depends_on","direction":"out","depth":1,"summary":"s","status":"NEW"}"#
    );
}
