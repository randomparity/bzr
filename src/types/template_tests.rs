use crate::types::template::BugTemplate;

fn full_template() -> BugTemplate {
    BugTemplate {
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
    }
}

#[test]
fn empty_template_has_no_bug_create_defaults() {
    let template = BugTemplate {
        product: None,
        component: None,
        version: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        description: None,
        url: None,
        whiteboard: None,
        target_milestone: None,
        deadline: None,
        cc: Vec::new(),
        keywords: Vec::new(),
        groups: Vec::new(),
        flags: Vec::new(),
    };

    assert!(template.is_empty());
}

#[test]
fn merge_from_replaces_only_supplied_fields() {
    let mut template = full_template();
    let updates = BugTemplate {
        product: Some("updated".into()),
        component: None,
        version: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        description: None,
        url: None,
        whiteboard: None,
        target_milestone: None,
        deadline: None,
        cc: Vec::new(),
        keywords: vec!["regression".into()],
        groups: Vec::new(),
        flags: vec!["review+".into()],
    };

    template.merge_from(&updates);

    assert_eq!(template.product.as_deref(), Some("updated"));
    assert_eq!(template.component.as_deref(), Some("c"));
    assert_eq!(template.cc, vec!["cc@example.com"]);
    assert_eq!(template.keywords, vec!["regression"]);
    assert_eq!(template.flags, vec!["review+"]);
}

#[test]
fn clear_field_resets_supported_names_and_aliases() {
    let mut template = full_template();

    for name in BugTemplate::clearable_fields() {
        assert!(template.clear_field(name), "{name} should be clearable");
    }

    assert!(template.is_empty());
    assert!(!template.clear_field("bogus"));
}
