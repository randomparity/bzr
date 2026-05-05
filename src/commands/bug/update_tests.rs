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
