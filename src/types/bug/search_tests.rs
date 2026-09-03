#![expect(clippy::unwrap_used, clippy::panic)]

use super::*;

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
    assert_eq!(
        by_struct("whiteboard").negation_operator,
        NegationOp::NotSubstring
    );
    assert_eq!(by_struct("url").negation_operator, NegationOp::NotSubstring);
    for f in ["assigned_to", "creator", "qa_contact"] {
        assert_eq!(
            by_struct(f).negation_operator,
            NegationOp::NoWordsSubstring,
            "role field {f} should use NoWordsSubstring"
        );
    }
    for f in [
        "product",
        "component",
        "status",
        "priority",
        "severity",
        "target_milestone",
        "version",
        "op_sys",
        "platform",
        "resolution",
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
    assert_eq!(NegationOp::NoWordsSubstring.as_str(), "nowordssubstr");
}

#[test]
fn invalid_role_negation_rejects_only_zero_word_negative_values() {
    for params in [
        SearchParams {
            assigned_to: vec!["!".into()],
            ..Default::default()
        },
        SearchParams {
            creator: vec!["! , \t".into()],
            ..Default::default()
        },
        SearchParams {
            qa_contact: vec!["!,,".into()],
            ..Default::default()
        },
    ] {
        assert!(params.invalid_role_negation().is_some());
    }

    for params in [
        SearchParams {
            assigned_to: vec!["!alice".into()],
            ..Default::default()
        },
        SearchParams {
            creator: vec!["! alice, bob ".into()],
            ..Default::default()
        },
        SearchParams {
            qa_contact: vec![String::new()],
            ..Default::default()
        },
    ] {
        assert!(params.invalid_role_negation().is_none());
    }
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
