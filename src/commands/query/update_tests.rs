#![expect(clippy::unwrap_used)]

#[test]
fn clear_query_field_handles_every_name() {
    let mut q = crate::types::SavedQuery {
        product: vec!["p".into()],
        component: vec!["c".into()],
        status: vec!["s".into()],
        assignee: vec!["a".into()],
        creator: vec!["cr".into()],
        priority: vec!["pr".into()],
        severity: vec!["se".into()],
        whiteboard: vec!["w".into()],
        target_milestone: vec!["t".into()],
        version: vec!["v".into()],
        op_sys: vec!["o".into()],
        platform: vec!["pl".into()],
        resolution: vec!["r".into()],
        qa_contact: vec!["q".into()],
        url: vec!["u".into()],
        quicksearch: Some("x".into()),
        limit: Some(5),
        fields: Some("f".into()),
        exclude_fields: Some("e".into()),
        creation_time: Some("2026-01-01".into()),
        last_change_time: Some("2026-02-01".into()),
        order: Some("bug_id".into()),
        ..Default::default()
    };
    for name in [
        "product",
        "component",
        "status",
        "assignee",
        "creator",
        "priority",
        "severity",
        "whiteboard",
        "target-milestone",
        "version",
        "op-sys",
        "platform",
        "resolution",
        "qa-contact",
        "url",
        "search",
        "limit",
        "fields",
        "exclude-fields",
        "created-since",
        "changed-since",
        "sort",
        "order",
    ] {
        super::clear_query_field(&mut q, name).unwrap();
    }
    assert!(!q.has_filters(), "every filter cleared");
    assert!(q.limit.is_none() && q.fields.is_none() && q.order.is_none());
}
