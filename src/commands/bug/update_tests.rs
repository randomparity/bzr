#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::BugAction;
use crate::test_helpers::{capture_stdout, setup_test_env};
use crate::types::OutputFormat;

fn make_update_action(ids: Vec<u64>) -> BugAction {
    BugAction::Update {
        ids,
        status: Some("RESOLVED".into()),
        resolution: Some("FIXED".into()),
        assignee: None,
        priority: None,
        severity: None,
        summary: None,
        whiteboard: None,
        flag: vec![],
        blocks_add: vec![],
        blocks_remove: vec![],
        depends_on_add: vec![],
        depends_on_remove: vec![],
        keywords_add: vec![],
        keywords_remove: vec![],
        cc_add: vec![],
        cc_remove: vec![],
        groups_add: vec![],
        groups_remove: vec![],
        see_also_add: vec![],
        see_also_remove: vec![],
    }
}

#[derive(Default)]
struct UpdateLists<'a> {
    keywords_add: Vec<&'a str>,
    keywords_remove: Vec<&'a str>,
    cc_add: Vec<&'a str>,
    cc_remove: Vec<&'a str>,
    groups_add: Vec<&'a str>,
    groups_remove: Vec<&'a str>,
    see_also_add: Vec<&'a str>,
    see_also_remove: Vec<&'a str>,
}

fn make_update_action_with_lists(lists: UpdateLists<'_>) -> BugAction {
    let to_strings = |v: Vec<&str>| v.into_iter().map(String::from).collect();
    BugAction::Update {
        ids: vec![1],
        status: None,
        resolution: None,
        assignee: None,
        priority: None,
        severity: None,
        summary: None,
        whiteboard: None,
        flag: vec![],
        blocks_add: vec![],
        blocks_remove: vec![],
        depends_on_add: vec![],
        depends_on_remove: vec![],
        keywords_add: to_strings(lists.keywords_add),
        keywords_remove: to_strings(lists.keywords_remove),
        cc_add: to_strings(lists.cc_add),
        cc_remove: to_strings(lists.cc_remove),
        groups_add: to_strings(lists.groups_add),
        groups_remove: to_strings(lists.groups_remove),
        see_also_add: to_strings(lists.see_also_add),
        see_also_remove: to_strings(lists.see_also_remove),
    }
}

async fn mock_put_bug_ok(mock: &wiremock::MockServer, id: u64) {
    Mock::given(method("PUT"))
        .and(path(format!("/rest/bug/{id}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": id, "changes": {}}]})),
        )
        .expect(1)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn bug_update_sends_put() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_put_bug_ok(&mock, 42).await;

    let action = make_update_action(vec![42]);
    let (result, output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["action"], "updated");
    assert_eq!(parsed["id"], 42);
}

#[tokio::test]
async fn bug_update_batch_mixed_results() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // First id succeeds, second id fails — exercises update_batch and
    // print_batch_result, including the BatchPartialFailure path.
    mock_put_bug_ok(&mock, 1).await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/2"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .expect(1)
        .mount(&mock)
        .await;

    let action = make_update_action(vec![1, 2]);
    let (result, output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(matches!(
        result,
        Err(crate::error::BzrError::BatchPartialFailure {
            succeeded: 1,
            failed: 1,
        })
    ));
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["succeeded"], serde_json::json!([1]));
    assert_eq!(parsed["failed"][0]["id"], 2);
}

#[tokio::test]
async fn bug_update_batch_table_format_all_succeed() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // Table format path through print_batch_result with no failures.
    mock_put_bug_ok(&mock, 1).await;
    mock_put_bug_ok(&mock, 2).await;

    let action = make_update_action(vec![1, 2]);
    let (result, output) = capture_stdout(crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Table,
        None,
    ))
    .await;
    assert!(result.is_ok());
    assert!(output.contains("Updated bugs:"));
    assert!(output.contains("#1"));
    assert!(output.contains("#2"));
}

#[test]
fn build_update_params_populates_string_lists() {
    let action = make_update_action_with_lists(UpdateLists {
        keywords_add: vec!["fix-needed"],
        cc_add: vec!["alice@example.com"],
        groups_remove: vec!["secret"],
        see_also_add: vec!["https://example.com/issue/1"],
        ..UpdateLists::default()
    });
    let (ids, params) = super::build_update_params(&action).unwrap();
    assert_eq!(ids, vec![1]);
    assert_eq!(params.keywords.add, vec!["fix-needed"]);
    assert_eq!(params.cc.add, vec!["alice@example.com"]);
    assert_eq!(params.groups.remove, vec!["secret"]);
    assert_eq!(params.see_also.add, vec!["https://example.com/issue/1"]);
}

#[test]
fn build_update_params_trims_string_list_values() {
    let action = make_update_action_with_lists(UpdateLists {
        keywords_add: vec!["  fix-needed  "],
        see_also_add: vec!["  https://example.com/issue/1  "],
        ..UpdateLists::default()
    });
    let (_ids, params) = super::build_update_params(&action).unwrap();
    assert_eq!(params.keywords.add, vec!["fix-needed"]);
    assert_eq!(params.see_also.add, vec!["https://example.com/issue/1"]);
}

#[test]
fn build_update_params_populates_keywords_remove() {
    let action = make_update_action_with_lists(UpdateLists {
        keywords_remove: vec!["stale"],
        ..UpdateLists::default()
    });
    let (_ids, params) = super::build_update_params(&action).unwrap();
    assert_eq!(params.keywords.remove, vec!["stale"]);
    assert!(params.keywords.add.is_empty());
}

#[test]
fn build_update_params_populates_cc_remove() {
    let action = make_update_action_with_lists(UpdateLists {
        cc_remove: vec!["bob@example.com"],
        ..UpdateLists::default()
    });
    let (_ids, params) = super::build_update_params(&action).unwrap();
    assert_eq!(params.cc.remove, vec!["bob@example.com"]);
    assert!(params.cc.add.is_empty());
}

#[test]
fn build_update_params_populates_groups_add() {
    let action = make_update_action_with_lists(UpdateLists {
        groups_add: vec!["secret"],
        ..UpdateLists::default()
    });
    let (_ids, params) = super::build_update_params(&action).unwrap();
    assert_eq!(params.groups.add, vec!["secret"]);
    assert!(params.groups.remove.is_empty());
}

#[test]
fn build_update_params_populates_see_also_remove() {
    let action = make_update_action_with_lists(UpdateLists {
        see_also_remove: vec!["https://example.com/issue/2"],
        ..UpdateLists::default()
    });
    let (_ids, params) = super::build_update_params(&action).unwrap();
    assert_eq!(params.see_also.remove, vec!["https://example.com/issue/2"]);
    assert!(params.see_also.add.is_empty());
}

#[test]
fn build_update_params_rejects_empty_keyword() {
    let action = make_update_action_with_lists(UpdateLists {
        keywords_add: vec!["", "fix-needed"],
        ..UpdateLists::default()
    });
    let err = super::build_update_params(&action).unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains("keywords-add")),
        "expected InputValidation naming keywords-add, got {err:?}"
    );
}

#[test]
fn build_update_params_rejects_whitespace_only_cc() {
    let action = make_update_action_with_lists(UpdateLists {
        cc_add: vec!["   "],
        ..UpdateLists::default()
    });
    let err = super::build_update_params(&action).unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains("cc-add")),
        "expected InputValidation naming cc-add, got {err:?}"
    );
}

#[test]
fn build_update_params_rejects_empty_groups_add() {
    let action = make_update_action_with_lists(UpdateLists {
        groups_add: vec![""],
        ..UpdateLists::default()
    });
    let err = super::build_update_params(&action).unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains("groups-add")),
        "expected InputValidation naming groups-add, got {err:?}"
    );
}

#[test]
fn build_update_params_rejects_whitespace_see_also_remove() {
    let action = make_update_action_with_lists(UpdateLists {
        see_also_remove: vec!["   "],
        ..UpdateLists::default()
    });
    let err = super::build_update_params(&action).unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains("see-also-remove")),
        "expected InputValidation naming see-also-remove, got {err:?}"
    );
}
