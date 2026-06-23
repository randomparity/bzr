#![expect(clippy::unwrap_used)]

#[test]
fn clear_template_field_handles_every_name() {
    let mut t = crate::types::BugTemplate {
        product: Some("p".into()),
        component: Some("c".into()),
        version: Some("v".into()),
        priority: Some("pr".into()),
        severity: Some("s".into()),
        assignee: Some("a".into()),
        op_sys: Some("o".into()),
        rep_platform: Some("rp".into()),
        description: Some("d".into()),
        url: Some("u".into()),
        whiteboard: Some("w".into()),
        target_milestone: Some("tm".into()),
        deadline: Some("2026-12-31".into()),
        cc: vec!["cc@example.com".into()],
        keywords: vec!["k".into()],
        groups: vec!["g".into()],
        flags: vec!["review?".into()],
    };
    for name in [
        "product",
        "component",
        "version",
        "priority",
        "severity",
        "assignee",
        "op-sys",
        "rep-platform",
        "description",
        "url",
        "whiteboard",
        "target-milestone",
        "deadline",
        "cc",
        "keywords",
        "groups",
        "flag",
        "flags",
    ] {
        super::clear_template_field(&mut t, name).unwrap();
    }
    assert!(super::template_is_empty(&t));
}
