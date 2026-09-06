#![expect(clippy::unwrap_used)]

use super::*;
use crate::types::{FlagStatus, FlagUpdate};

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

#[test]
fn update_bug_params_omits_comment_tags_when_empty() {
    let params = UpdateBugParams::default();
    let json = serde_json::to_value(&params).unwrap();
    assert!(
        json.get("comment_tags").is_none(),
        "expected no comment_tags key when empty, got: {json}"
    );
}

#[test]
fn update_bug_params_serializes_comment_tags_as_sibling_of_comment() {
    let params = UpdateBugParams {
        comment: Some(CommentUpdate {
            body: "tagged".into(),
            is_private: false,
        }),
        comment_tags: vec!["triaged".into(), "needs-review".into()],
        ..Default::default()
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["comment"], serde_json::json!({"body": "tagged"}));
    assert_eq!(
        json["comment_tags"],
        serde_json::json!(["triaged", "needs-review"])
    );
}

#[test]
fn update_bug_params_omits_minor_update_when_false() {
    let params = UpdateBugParams::default();
    let json = serde_json::to_value(&params).unwrap();
    assert!(
        json.get("minor_update").is_none(),
        "expected no minor_update key when false, got: {json}"
    );
}

#[test]
fn update_bug_params_serializes_minor_update_when_true() {
    let params = UpdateBugParams {
        minor_update: true,
        ..Default::default()
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["minor_update"], serde_json::Value::Bool(true));
}

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
fn create_params_minimal_matches_wire_contract() {
    let json = serde_json::to_value(minimal_create_params()).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "product": "Prod",
            "component": "Comp",
            "summary": "Sum",
            "version": "1.0",
        })
    );
}

#[test]
fn create_params_all_fields_match_wire_contract() {
    let params = CreateBugParams {
        description: Some("full description".into()),
        priority: Some("high".into()),
        severity: Some("major".into()),
        assigned_to: Some("owner@example.com".into()),
        op_sys: Some("Linux".into()),
        platform: Some("x86_64".into()),
        alias: Some("my-alias".into()),
        url: Some("https://example.com/repro".into()),
        whiteboard: Some("needs-triage".into()),
        target_milestone: Some("M1".into()),
        deadline: Some("2026-12-31".into()),
        blocks: vec![12, 13],
        depends_on: vec![14, 15],
        cc: vec!["a@example.com".into(), "b@example.com".into()],
        keywords: vec!["regression".into()],
        groups: vec!["security".into()],
        flags: vec![FlagUpdate {
            name: "review".into(),
            status: FlagStatus::Grant,
            requestee: Some("reviewer@example.com".into()),
        }],
        comment_tags: vec!["triaged".into()],
        ..minimal_create_params()
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "product": "Prod",
            "component": "Comp",
            "summary": "Sum",
            "version": "1.0",
            "description": "full description",
            "priority": "high",
            "severity": "major",
            "assigned_to": "owner@example.com",
            "op_sys": "Linux",
            "platform": "x86_64",
            "alias": "my-alias",
            "url": "https://example.com/repro",
            "whiteboard": "needs-triage",
            "target_milestone": "M1",
            "deadline": "2026-12-31",
            "blocks": [12, 13],
            "depends_on": [14, 15],
            "cc": ["a@example.com", "b@example.com"],
            "keywords": ["regression"],
            "groups": ["security"],
            "flags": [{
                "name": "review",
                "status": "+",
                "requestee": "reviewer@example.com",
            }],
            "comment_tags": ["triaged"],
        })
    );
}

#[test]
fn create_params_omits_empty_comment_tags() {
    let json = serde_json::to_value(minimal_create_params()).unwrap();
    assert!(
        json.get("comment_tags").is_none(),
        "empty comment_tags must omit, got: {json}"
    );
}

#[test]
fn create_params_ordinary_empty_groups_are_omitted() {
    let params = minimal_create_params();
    let json = serde_json::to_value(params).unwrap();

    assert!(
        json.get("groups").is_none(),
        "empty public groups must omit"
    );
}

#[test]
fn create_params_ordinary_non_empty_groups_serialize() {
    let params = CreateBugParams {
        groups: vec!["security".into()],
        ..minimal_create_params()
    };
    let json = serde_json::to_value(params).unwrap();

    assert_eq!(json["groups"], serde_json::json!(["security"]));
}

#[test]
fn create_params_structured_empty_groups_serialize() {
    let mut params = minimal_create_params();
    params.set_groups_from_structured_input(vec![]);
    let json = serde_json::to_value(params).unwrap();

    assert_eq!(json["groups"], serde_json::json!([]));
}

#[test]
fn create_params_groups_conflict_serializes_current_values_once() {
    let mut params = minimal_create_params();
    params.set_groups_from_structured_input(vec![]);
    params.groups = vec!["security".into()];

    let raw = serde_json::to_string(&params).unwrap();
    assert_eq!(raw.match_indices("\"groups\"").count(), 1, "raw: {raw}");
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(json["groups"], serde_json::json!(["security"]));
}
