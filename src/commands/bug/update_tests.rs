#![expect(clippy::unwrap_used, clippy::expect_used)]

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::BugAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn make_update_action(ids: Vec<u64>) -> BugAction {
    BugAction::Update {
        ids,
        status: Some("RESOLVED".into()),
        resolution: Some("FIXED".into()),
        dupe_of: None,
        alias: None,
        deadline: None,
        estimated_time: None,
        remaining_time: None,
        work_time: None,
        reset_assigned_to: false,
        reset_qa_contact: false,
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
        comment: None,
        comment_file: None,
        comment_private: false,
    }
}

fn make_update_action_with_dupe_of(id: u64, dupe_of: u64) -> BugAction {
    BugAction::Update {
        ids: vec![id],
        status: None,
        resolution: None,
        dupe_of: Some(dupe_of),
        alias: None,
        deadline: None,
        estimated_time: None,
        remaining_time: None,
        work_time: None,
        reset_assigned_to: false,
        reset_qa_contact: false,
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
        comment: None,
        comment_file: None,
        comment_private: false,
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
        dupe_of: None,
        alias: None,
        deadline: None,
        estimated_time: None,
        remaining_time: None,
        work_time: None,
        reset_assigned_to: false,
        reset_qa_contact: false,
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
        comment: None,
        comment_file: None,
        comment_private: false,
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
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut __io.writers())
            .await;
    let output = __io.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["action"], "updated");
    assert_eq!(parsed["id"], 42);
}

#[tokio::test]
async fn bug_update_sends_dupe_of_body() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .and(body_json(serde_json::json!({"dupe_of": 99})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": 42, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let action = make_update_action_with_dupe_of(42, 99);
    let mut io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut io.writers())
            .await;

    assert!(result.is_ok());
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
    let mut __io2 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io2.writers(),
    )
    .await;
    let output = __io2.out_str().to_string();
    assert!(matches!(
        result,
        Err(crate::error::BzrError::BatchPartialFailure {
            succeeded: 1,
            failed: 1,
        })
    ));
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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
    let mut __io3 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Table,
        None,
        &mut __io3.writers(),
    )
    .await;
    let output = __io3.out_str().to_string();
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
fn build_update_params_populates_dupe_of() {
    let action = make_update_action_with_dupe_of(42, 99);
    let (ids, params) = super::build_update_params(&action).unwrap();

    assert_eq!(ids, vec![42]);
    assert_eq!(params.dupe_of, Some(99));
    assert!(params.status.is_none());
    assert!(params.resolution.is_none());
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
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains(super::FLAG_KEYWORDS_ADD)),
        "expected InputValidation naming {}, got {err:?}",
        super::FLAG_KEYWORDS_ADD
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
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains(super::FLAG_CC_ADD)),
        "expected InputValidation naming {}, got {err:?}",
        super::FLAG_CC_ADD
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
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains(super::FLAG_GROUPS_ADD)),
        "expected InputValidation naming {}, got {err:?}",
        super::FLAG_GROUPS_ADD
    );
}

#[test]
fn build_update_params_rejects_whitespace_only_see_also_remove() {
    let action = make_update_action_with_lists(UpdateLists {
        see_also_remove: vec!["   "],
        ..UpdateLists::default()
    });
    let err = super::build_update_params(&action).unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(msg) if msg.contains(super::FLAG_SEE_ALSO_REMOVE)),
        "expected InputValidation naming {}, got {err:?}",
        super::FLAG_SEE_ALSO_REMOVE
    );
}

fn make_update_action_with_comment(
    ids: Vec<u64>,
    comment: Option<&str>,
    comment_file: Option<&std::path::Path>,
    comment_private: bool,
) -> BugAction {
    BugAction::Update {
        ids,
        status: None,
        resolution: None,
        dupe_of: None,
        alias: None,
        deadline: None,
        estimated_time: None,
        remaining_time: None,
        work_time: None,
        reset_assigned_to: false,
        reset_qa_contact: false,
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
        comment: comment.map(String::from),
        comment_file: comment_file.map(std::path::PathBuf::from),
        comment_private,
    }
}

#[test]
fn build_update_params_carries_public_comment() {
    let action = make_update_action_with_comment(vec![1], Some("hello"), None, false);
    let (_ids, params) = super::build_update_params(&action).unwrap();
    let comment = params.comment.expect("comment populated");
    assert_eq!(comment.body, "hello");
    assert!(!comment.is_private);
}

#[test]
fn build_update_params_carries_private_comment() {
    let action = make_update_action_with_comment(vec![1], Some("secret"), None, true);
    let (_ids, params) = super::build_update_params(&action).unwrap();
    let comment = params.comment.expect("comment populated");
    assert_eq!(comment.body, "secret");
    assert!(comment.is_private);
}

#[test]
fn build_update_params_omits_comment_when_unspecified() {
    let action = make_update_action_with_comment(vec![1], None, None, false);
    let (_ids, params) = super::build_update_params(&action).unwrap();
    assert!(params.comment.is_none());
}

#[test]
fn build_update_params_rejects_private_without_body() {
    let action = make_update_action_with_comment(vec![1], None, None, true);
    let err = super::build_update_params(&action).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("--comment-private"),
        "error should mention the flag: {msg}"
    );
    assert!(matches!(err, crate::error::BzrError::InputValidation(_)));
}

#[test]
fn build_update_params_rejects_whitespace_only_comment() {
    let action = make_update_action_with_comment(vec![1], Some("   \n\t"), None, false);
    let err = super::build_update_params(&action).unwrap_err();
    assert!(matches!(
        err,
        crate::error::BzrError::InputValidation(ref m) if m.contains("empty comment")
    ));
}

#[test]
fn build_update_params_reads_comment_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("body.txt");
    std::fs::write(&path, "from a file").unwrap();
    let action = make_update_action_with_comment(vec![1], None, Some(&path), false);
    let (_ids, params) = super::build_update_params(&action).unwrap();
    let comment = params.comment.expect("comment populated");
    assert_eq!(comment.body, "from a file");
    assert!(!comment.is_private);
}

#[test]
fn build_update_params_comment_file_with_private() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("body.txt");
    std::fs::write(&path, "private body").unwrap();
    let action = make_update_action_with_comment(vec![1], None, Some(&path), true);
    let (_ids, params) = super::build_update_params(&action).unwrap();
    let comment = params.comment.expect("comment populated");
    assert!(comment.is_private);
}

#[test]
fn build_update_params_rejects_missing_comment_file() {
    let path = std::path::Path::new("/nonexistent/bzr-issue-161-test.txt");
    let action = make_update_action_with_comment(vec![1], None, Some(path), false);
    let err = super::build_update_params(&action).unwrap_err();
    let msg = err.to_string();
    assert!(matches!(err, crate::error::BzrError::InputValidation(_)));
    assert!(
        msg.contains("/nonexistent/bzr-issue-161-test.txt"),
        "error should include path: {msg}"
    );
}

#[test]
fn build_update_params_rejects_non_utf8_comment_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("body.bin");
    std::fs::write(&path, [0xff_u8, 0xfe, 0xfd]).unwrap();
    let action = make_update_action_with_comment(vec![1], None, Some(&path), false);
    let err = super::build_update_params(&action).unwrap_err();
    assert!(matches!(err, crate::error::BzrError::InputValidation(_)));
}

#[test]
fn build_update_params_rejects_whitespace_only_comment_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("body.txt");
    std::fs::write(&path, "   \n\t  \n").unwrap();
    let action = make_update_action_with_comment(vec![1], None, Some(&path), false);
    let err = super::build_update_params(&action).unwrap_err();
    assert!(matches!(
        err,
        crate::error::BzrError::InputValidation(ref m) if m.contains("empty comment")
    ));
}

#[tokio::test]
async fn bug_update_table_output_with_comment_single() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_put_bug_ok(&mock, 42).await;

    let action = make_update_action_with_comment(vec![42], Some("hi"), None, false);
    let mut __io4 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Table,
        None,
        &mut __io4.writers(),
    )
    .await;
    let output = __io4.out_str().to_string();
    assert!(result.is_ok());
    assert!(
        output.contains("Updated bug #42 (with comment)"),
        "expected '(with comment)' suffix; got: {output}"
    );
}

#[tokio::test]
async fn bug_update_table_output_no_comment_single() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_put_bug_ok(&mock, 42).await;

    let action = make_update_action(vec![42]);
    let mut __io5 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Table,
        None,
        &mut __io5.writers(),
    )
    .await;
    let output = __io5.out_str().to_string();
    assert!(result.is_ok());
    assert!(output.contains("Updated bug #42"));
    assert!(
        !output.contains("(with comment)"),
        "no comment was posted; suffix should be absent: {output}"
    );
}

#[tokio::test]
async fn bug_update_table_output_with_comment_batch_all_succeed() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_put_bug_ok(&mock, 1).await;
    mock_put_bug_ok(&mock, 2).await;

    let action = make_update_action_with_comment(vec![1, 2], Some("batch comment"), None, false);
    let mut __io6 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Table,
        None,
        &mut __io6.writers(),
    )
    .await;
    let output = __io6.out_str().to_string();
    assert!(result.is_ok());
    assert!(output.contains("Updated bugs:"));
    assert!(output.contains("(with comment)"));
}

#[tokio::test]
async fn bug_update_json_output_unchanged_with_comment() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_put_bug_ok(&mock, 42).await;

    let action = make_update_action_with_comment(vec![42], Some("hi"), None, false);
    let mut __io7 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io7.writers(),
    )
    .await;
    let output = __io7.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["action"], "updated");
    assert_eq!(parsed["id"], 42);
    assert!(
        parsed.get("comment").is_none() && parsed.get("with_comment").is_none(),
        "JSON schema should not gain a comment-related key: {parsed}"
    );
}
