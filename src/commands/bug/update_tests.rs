#![expect(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use wiremock::matchers::{body_json, body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::BugAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

/// Borrow the inner `UpdateArgs` from a `BugAction::Update` test fixture.
fn as_update_args(action: &BugAction) -> &crate::cli::UpdateArgs {
    match action {
        BugAction::Update(args) => args,
        _ => panic!("expected BugAction::Update"),
    }
}

fn make_update_action(ids: Vec<u64>) -> BugAction {
    BugAction::Update(crate::cli::UpdateArgs {
        ids,
        status: Some("RESOLVED".into()),
        resolution: Some("FIXED".into()),
        ..Default::default()
    })
}

fn make_update_action_with_dupe_of(id: u64, dupe_of: u64) -> BugAction {
    BugAction::Update(crate::cli::UpdateArgs {
        ids: vec![id],
        dupe_of: Some(dupe_of),
        ..Default::default()
    })
}

fn make_update_action_with_scalar_parity_fields() -> BugAction {
    BugAction::Update(crate::cli::UpdateArgs {
        ids: vec![42],
        alias: Some("short-name".into()),
        deadline: Some("2026-12-31".into()),
        estimated_time: Some(3.5),
        remaining_time: Some(1.25),
        work_time: Some(0.5),
        reset_assigned_to: true,
        reset_qa_contact: true,
        url: Some("https://example.com/repro".into()),
        target_milestone: Some("5.0".into()),
        ..Default::default()
    })
}

fn update_ids_mut(action: &mut BugAction) -> Option<&mut Vec<u64>> {
    let BugAction::Update(crate::cli::UpdateArgs { ids, .. }) = action else {
        return None;
    };
    Some(ids)
}

fn update_deadline_mut(action: &mut BugAction) -> Option<&mut Option<String>> {
    let BugAction::Update(crate::cli::UpdateArgs { deadline, .. }) = action else {
        return None;
    };
    Some(deadline)
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
    BugAction::Update(crate::cli::UpdateArgs {
        ids: vec![1],
        keywords_add: to_strings(lists.keywords_add),
        keywords_remove: to_strings(lists.keywords_remove),
        cc_add: to_strings(lists.cc_add),
        cc_remove: to_strings(lists.cc_remove),
        groups_add: to_strings(lists.groups_add),
        groups_remove: to_strings(lists.groups_remove),
        see_also_add: to_strings(lists.see_also_add),
        see_also_remove: to_strings(lists.see_also_remove),
        ..Default::default()
    })
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

/// Mount a GET mock returning a bug with the given `last_change_time`, for the
/// `--expect-unchanged-since` re-read.
async fn mock_get_bug_lct(mock: &wiremock::MockServer, id: u64, lct: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/bug/{id}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": id, "last_change_time": lct}]})),
        )
        .mount(mock)
        .await;
}

async fn received_put_count(mock: &wiremock::MockServer) -> usize {
    mock.received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|request| request.method.as_str() == "PUT")
        .count()
}

fn update_action_expect_unchanged(ids: Vec<u64>, since: &str) -> BugAction {
    let mut action = make_update_action(ids);
    if let BugAction::Update(crate::cli::UpdateArgs {
        expect_unchanged_since,
        ..
    }) = &mut action
    {
        *expect_unchanged_since = Some(since.to_string());
    }
    action
}

fn from_json_update_action(path: &str) -> BugAction {
    BugAction::Update(crate::cli::UpdateArgs {
        from_json: Some(path.to_string()),
        ..Default::default()
    })
}

fn from_json_update_action_with_ids(ids: Vec<u64>, path: &str) -> BugAction {
    BugAction::Update(crate::cli::UpdateArgs {
        from_json: Some(path.to_string()),
        ids,
        ..Default::default()
    })
}

fn write_json_file(tmp: &tempfile::TempDir, json: &str) -> String {
    let path = tmp.path().join("update-input.json");
    std::fs::write(&path, json).unwrap();
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn bug_update_from_json_object_uses_positional_id() {
    let (_lock, mock, tmp) = setup_test_env().await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .and(body_json(serde_json::json!({"status": "ASSIGNED"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": 42, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"status":"ASSIGNED"}"#;
    let action = from_json_update_action_with_ids(vec![42], &write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "JSON object update should succeed: {result:?}"
    );
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["action"], "updated");
    assert_eq!(parsed["id"], 42);
}

#[tokio::test]
async fn bug_update_from_json_cli_flag_overrides_json_field() {
    let (_lock, mock, tmp) = setup_test_env().await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .and(body_json(serde_json::json!({"status": "ASSIGNED"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": 42, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"id":42,"status":"NEW"}"#;
    let mut action = from_json_update_action(&write_json_file(&tmp, json));
    if let BugAction::Update(crate::cli::UpdateArgs { status, .. }) = &mut action {
        *status = Some("ASSIGNED".into());
    }
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "CLI status should override JSON: {result:?}"
    );
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["action"], "updated");
    assert_eq!(parsed["id"], 42);
}

#[tokio::test]
async fn bug_update_from_json_list_fields_use_add_remove_shapes() {
    let (_lock, mock, tmp) = setup_test_env().await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .and(body_partial_json(serde_json::json!({
            "keywords": {"add": ["fix-needed"], "remove": ["stale"]},
            "blocks": {"add": [100]},
            "depends_on": {"remove": [50]},
            "see_also": {"add": ["https://example.com/issue/1"]},
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": 42, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{
        "keywords_add":["fix-needed"],
        "keywords_remove":["stale"],
        "blocks_add":[100],
        "depends_on_remove":[50],
        "see_also_add":["https://example.com/issue/1"]
    }"#;
    let action = from_json_update_action_with_ids(vec![42], &write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "JSON list deltas should update: {result:?}");
}

#[tokio::test]
async fn bug_update_from_json_array_batches_per_id() {
    let (_lock, mock, tmp) = setup_test_env().await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/1"))
        .and(body_json(serde_json::json!({"status": "ASSIGNED"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": 1, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/2"))
        .and(body_json(serde_json::json!({"priority": "high"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": 2, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"[{"id":1,"status":"ASSIGNED"},{"id":2,"priority":"high"}]"#;
    let action = from_json_update_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "array update should succeed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["succeeded"], serde_json::json!([1, 2]));
    assert_eq!(parsed["failed"], serde_json::json!([]));
}

#[tokio::test]
async fn bug_update_from_json_single_element_array_returns_batch_shape() {
    let (_lock, mock, tmp) = setup_test_env().await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/8"))
        .and(body_json(serde_json::json!({"status": "ASSIGNED"})))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": 8, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"[{"id":8,"status":"ASSIGNED"}]"#;
    let action = from_json_update_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "one-element array should update: {result:?}"
    );
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["succeeded"], serde_json::json!([8]));
    assert_eq!(parsed["failed"], serde_json::json!([]));
    assert!(
        parsed.get("id").is_none(),
        "array input must not use the single-object shape"
    );
}

#[tokio::test]
async fn bug_update_from_json_array_dry_run_emits_single_object_and_no_write() {
    let (_lock, mock, tmp) = setup_test_env().await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let json = r#"[{"id":1,"status":"ASSIGNED"},{"id":2,"priority":"high"}]"#;
    let action = from_json_update_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "array dry-run should succeed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([1, 2]));
    assert_eq!(parsed["changes"].as_array().unwrap().len(), 2);
    assert_eq!(received_put_count(&mock).await, 0);
}

#[tokio::test]
async fn bug_update_from_json_array_partial_failure_exits_11() {
    let (_lock, mock, tmp) = setup_test_env().await;
    mock_put_bug_ok(&mock, 1).await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/2"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"[{"id":1,"status":"ASSIGNED"},{"id":2,"status":"ASSIGNED"}]"#;
    let action = from_json_update_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(matches!(
        err,
        crate::error::BzrError::BatchPartialFailure {
            succeeded: 1,
            failed: 1
        }
    ));
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["succeeded"], serde_json::json!([1]));
    assert_eq!(parsed["failed"][0]["id"], 2);
}

#[tokio::test]
async fn bug_update_from_json_array_guard_failure_prevents_all_writes() {
    let (_lock, mock, tmp) = setup_test_env().await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": 1, "changes": {}}]})),
        )
        .mount(&mock)
        .await;
    mock_get_bug_lct(&mock, 2, "2026-06-21T12:00:01Z").await;

    let json = r#"[
        {"id":1,"status":"ASSIGNED"},
        {
            "id":2,
            "status":"ASSIGNED",
            "expect_unchanged_since":"2026-06-21T12:00:00Z"
        }
    ]"#;
    let action = from_json_update_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(matches!(
        err,
        crate::error::BzrError::BatchPartialFailure {
            succeeded: 0,
            failed: 1
        }
    ));
    let parsed: serde_json::Value = serde_json::from_str(io.out_str().trim()).unwrap();
    assert_eq!(parsed["succeeded"], serde_json::json!([]));
    assert_eq!(parsed["failed"][0]["id"], 2);
    assert_eq!(received_put_count(&mock).await, 0);
}

#[tokio::test]
async fn bug_update_from_json_comment_and_expect_guard() {
    let (_lock, mock, tmp) = setup_test_env().await;
    mock_get_bug_lct(&mock, 42, "2026-06-21T12:00:00Z").await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .and(body_partial_json(serde_json::json!({
            "status": "ASSIGNED",
            "comment": {
                "body": "ready",
                "is_private": true,
            },
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": 42, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{
        "id":42,
        "status":"ASSIGNED",
        "comment":"ready",
        "comment_private":true,
        "expect_unchanged_since":"2026-06-21T12:00:00Z"
    }"#;
    let action = from_json_update_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "JSON comment/concurrency update should succeed: {result:?}"
    );
}

#[tokio::test]
async fn bug_update_from_json_rejects_unknown_field() {
    let (_lock, mock, tmp) = setup_test_env().await;
    let json = r#"{"id":42,"status":"ASSIGNED","bogus":true}"#;
    let action = from_json_update_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(err, crate::error::BzrError::InputValidation(ref msg) if msg.contains("bogus") || msg.contains("unknown field")),
        "expected unknown-field validation, got {err:?}"
    );
    assert_eq!(received_put_count(&mock).await, 0);
}

#[tokio::test]
async fn bug_update_from_json_rejects_empty_array() {
    let (_lock, mock, tmp) = setup_test_env().await;
    let action = from_json_update_action(&write_json_file(&tmp, "[]"));
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(err, crate::error::BzrError::InputValidation(ref msg) if msg.contains("empty array")),
        "expected empty-array validation, got {err:?}"
    );
    assert_eq!(received_put_count(&mock).await, 0);
}

#[tokio::test]
async fn bug_update_from_json_rejects_array_item_without_id() {
    let (_lock, mock, tmp) = setup_test_env().await;
    let json = r#"[{"status":"ASSIGNED"}]"#;
    let action = from_json_update_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(err, crate::error::BzrError::InputValidation(ref msg) if msg.contains("id is required")),
        "expected array-item id validation, got {err:?}"
    );
    assert_eq!(received_put_count(&mock).await, 0);
}

#[tokio::test]
async fn bug_update_from_json_rejects_object_without_target_id() {
    let (_lock, mock, tmp) = setup_test_env().await;
    let json = r#"{"status":"ASSIGNED"}"#;
    let action = from_json_update_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(err, crate::error::BzrError::InputValidation(ref msg) if msg.contains("requires positional IDs") && msg.contains("id field")),
        "expected object target validation, got {err:?}"
    );
    assert_eq!(received_put_count(&mock).await, 0);
}

#[tokio::test]
async fn bug_update_from_json_rejects_mixed_positional_and_json_id() {
    let (_lock, mock, tmp) = setup_test_env().await;
    let json = r#"{"id":42,"status":"ASSIGNED"}"#;
    let action = from_json_update_action_with_ids(vec![43], &write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(err, crate::error::BzrError::InputValidation(ref msg) if msg.contains("id") && msg.contains("positional")),
        "expected mixed-id-source validation, got {err:?}"
    );
    assert_eq!(received_put_count(&mock).await, 0);
}

#[tokio::test]
async fn bug_update_from_json_rejects_dupe_of_with_status() {
    let (_lock, mock, tmp) = setup_test_env().await;
    let json = r#"{"id":42,"dupe_of":99,"status":"RESOLVED"}"#;
    let action = from_json_update_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(err, crate::error::BzrError::InputValidation(ref msg) if msg.contains("--dupe-of") && msg.contains("--status")),
        "expected dupe/status conflict validation, got {err:?}"
    );
    assert_eq!(received_put_count(&mock).await, 0);
}

#[tokio::test]
async fn bug_update_from_json_rejects_json_comment_file_stdin() {
    let (_lock, mock, tmp) = setup_test_env().await;
    let json = r#"{"id":42,"status":"ASSIGNED","comment_file":"-"}"#;
    let action = from_json_update_action(&write_json_file(&tmp, json));
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(err, crate::error::BzrError::InputValidation(ref msg) if msg.contains("comment_file") && msg.contains("stdin")),
        "expected comment_file stdin validation, got {err:?}"
    );
    assert_eq!(received_put_count(&mock).await, 0);
}

#[tokio::test]
async fn bug_update_from_json_cli_comment_overrides_json_comment_file_stdin() {
    let (_lock, mock, tmp) = setup_test_env().await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .and(body_partial_json(serde_json::json!({
            "status": "ASSIGNED",
            "comment": {
                "body": "ready",
            },
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": 42, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let json = r#"{"id":42,"status":"ASSIGNED","comment_file":"-"}"#;
    let mut action = from_json_update_action(&write_json_file(&tmp, json));
    let BugAction::Update(args) = &mut action else {
        panic!("expected update action");
    };
    args.comment = Some("ready".into());

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "CLI comment should override JSON comment_file stdin: {result:?}"
    );
    assert_eq!(received_put_count(&mock).await, 1);
}

#[tokio::test]
async fn bug_update_sends_put() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_put_bug_ok(&mock, 42).await;

    let action = make_update_action(vec![42]);
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    let output = __io.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["action"], "updated");
    assert_eq!(parsed["id"], 42);
}

#[tokio::test]
async fn bug_update_alias_multiple_ids_rejected_before_connect() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    let mut action = make_update_action_with_scalar_parity_fields();
    *update_ids_mut(&mut action).expect("expected update action") = vec![42, 43];

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(matches!(
        result,
        Err(crate::error::BzrError::InputValidation(ref msg)) if msg.contains("--alias")
    ));
    let received = mock.received_requests().await.unwrap();
    assert!(
        received.is_empty(),
        "expected validation before network I/O, got {} request(s)",
        received.len()
    );
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
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn bug_update_sends_url_and_target_milestone_body() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .and(body_json(serde_json::json!({
            "url": "https://example.com/repro",
            "target_milestone": "5.0"
        })))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": 42, "changes": {}}]})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::Update(crate::cli::UpdateArgs {
        ids: vec![42],
        url: Some("https://example.com/repro".into()),
        target_milestone: Some("5.0".into()),
        ..Default::default()
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
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
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Table, None),
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
    let (ids, params) = super::build_update_params(as_update_args(&action)).unwrap();
    assert_eq!(ids, vec![1]);
    assert_eq!(params.keywords.add, vec!["fix-needed"]);
    assert_eq!(params.cc.add, vec!["alice@example.com"]);
    assert_eq!(params.groups.remove, vec!["secret"]);
    assert_eq!(params.see_also.add, vec!["https://example.com/issue/1"]);
}

#[test]
fn build_update_params_populates_dupe_of() {
    let action = make_update_action_with_dupe_of(42, 99);
    let (ids, params) = super::build_update_params(as_update_args(&action)).unwrap();

    assert_eq!(ids, vec![42]);
    assert_eq!(params.dupe_of, Some(99));
    assert!(params.status.is_none());
    assert!(params.resolution.is_none());
}

#[test]
fn build_update_params_populates_scalar_parity_fields() {
    let action = make_update_action_with_scalar_parity_fields();
    let (_ids, params) = super::build_update_params(as_update_args(&action)).unwrap();

    assert_eq!(params.alias.as_deref(), Some("short-name"));
    assert_eq!(params.deadline.as_deref(), Some("2026-12-31"));
    assert_eq!(params.estimated_time, Some(3.5));
    assert_eq!(params.remaining_time, Some(1.25));
    assert_eq!(params.work_time, Some(0.5));
    assert_eq!(params.url.as_deref(), Some("https://example.com/repro"));
    assert_eq!(params.target_milestone.as_deref(), Some("5.0"));
    assert!(params.reset_assigned_to);
    assert!(params.reset_qa_contact);
}

#[test]
fn build_update_params_accepts_valid_deadline_verbatim() {
    let mut action = make_update_action(vec![42]);
    *update_deadline_mut(&mut action).expect("update action") = Some("2026-12-31".into());

    let (_ids, params) = super::build_update_params(as_update_args(&action)).unwrap();
    // Date-only deadlines must reach the server unchanged (no datetime expansion).
    assert_eq!(params.deadline.as_deref(), Some("2026-12-31"));
}

#[test]
fn build_update_params_rejects_invalid_deadline() {
    let mut action = make_update_action(vec![42]);
    *update_deadline_mut(&mut action).expect("update action") = Some("garbage".into());

    let err = super::build_update_params(as_update_args(&action)).unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(
        matches!(err, crate::error::BzrError::InputValidation(ref msg) if msg.contains("--deadline")),
        "expected --deadline validation error, got {err:?}"
    );
}

#[test]
fn build_update_params_rejects_alias_with_multiple_ids() {
    let mut action = make_update_action_with_scalar_parity_fields();
    *update_ids_mut(&mut action).expect("expected update action") = vec![42, 43];

    let err = super::build_update_params(as_update_args(&action)).unwrap_err();
    assert!(
        matches!(err, crate::error::BzrError::InputValidation(ref msg) if msg.contains("--alias")),
        "expected --alias validation error, got {err:?}"
    );
}

#[test]
fn build_update_params_trims_string_list_values() {
    let action = make_update_action_with_lists(UpdateLists {
        keywords_add: vec!["  fix-needed  "],
        see_also_add: vec!["  https://example.com/issue/1  "],
        ..UpdateLists::default()
    });
    let (_ids, params) = super::build_update_params(as_update_args(&action)).unwrap();
    assert_eq!(params.keywords.add, vec!["fix-needed"]);
    assert_eq!(params.see_also.add, vec!["https://example.com/issue/1"]);
}

#[test]
fn build_update_params_populates_keywords_remove() {
    let action = make_update_action_with_lists(UpdateLists {
        keywords_remove: vec!["stale"],
        ..UpdateLists::default()
    });
    let (_ids, params) = super::build_update_params(as_update_args(&action)).unwrap();
    assert_eq!(params.keywords.remove, vec!["stale"]);
    assert!(params.keywords.add.is_empty());
}

#[test]
fn build_update_params_populates_cc_remove() {
    let action = make_update_action_with_lists(UpdateLists {
        cc_remove: vec!["bob@example.com"],
        ..UpdateLists::default()
    });
    let (_ids, params) = super::build_update_params(as_update_args(&action)).unwrap();
    assert_eq!(params.cc.remove, vec!["bob@example.com"]);
    assert!(params.cc.add.is_empty());
}

#[test]
fn build_update_params_populates_groups_add() {
    let action = make_update_action_with_lists(UpdateLists {
        groups_add: vec!["secret"],
        ..UpdateLists::default()
    });
    let (_ids, params) = super::build_update_params(as_update_args(&action)).unwrap();
    assert_eq!(params.groups.add, vec!["secret"]);
    assert!(params.groups.remove.is_empty());
}

#[test]
fn build_update_params_populates_see_also_remove() {
    let action = make_update_action_with_lists(UpdateLists {
        see_also_remove: vec!["https://example.com/issue/2"],
        ..UpdateLists::default()
    });
    let (_ids, params) = super::build_update_params(as_update_args(&action)).unwrap();
    assert_eq!(params.see_also.remove, vec!["https://example.com/issue/2"]);
    assert!(params.see_also.add.is_empty());
}

#[test]
fn build_update_params_rejects_empty_keyword() {
    let action = make_update_action_with_lists(UpdateLists {
        keywords_add: vec!["", "fix-needed"],
        ..UpdateLists::default()
    });
    let err = super::build_update_params(as_update_args(&action)).unwrap_err();
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
    let err = super::build_update_params(as_update_args(&action)).unwrap_err();
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
    let err = super::build_update_params(as_update_args(&action)).unwrap_err();
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
    let err = super::build_update_params(as_update_args(&action)).unwrap_err();
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
    BugAction::Update(crate::cli::UpdateArgs {
        ids,
        comment: comment.map(String::from),
        comment_file: comment_file.map(std::path::PathBuf::from),
        comment_private,
        ..Default::default()
    })
}

#[test]
fn build_update_params_carries_public_comment() {
    let action = make_update_action_with_comment(vec![1], Some("hello"), None, false);
    let (_ids, params) = super::build_update_params(as_update_args(&action)).unwrap();
    let comment = params.comment.expect("comment populated");
    assert_eq!(comment.body, "hello");
    assert!(!comment.is_private);
}

#[test]
fn build_update_params_carries_private_comment() {
    let action = make_update_action_with_comment(vec![1], Some("secret"), None, true);
    let (_ids, params) = super::build_update_params(as_update_args(&action)).unwrap();
    let comment = params.comment.expect("comment populated");
    assert_eq!(comment.body, "secret");
    assert!(comment.is_private);
}

#[test]
fn build_update_params_omits_comment_when_unspecified() {
    let mut action = make_update_action_with_comment(vec![1], None, None, false);
    if let BugAction::Update(crate::cli::UpdateArgs { status, .. }) = &mut action {
        *status = Some("CONFIRMED".into());
    }
    let (_ids, params) = super::build_update_params(as_update_args(&action)).unwrap();
    assert!(params.comment.is_none());
}

#[test]
fn build_update_params_rejects_private_without_body() {
    let action = make_update_action_with_comment(vec![1], None, None, true);
    let err = super::build_update_params(as_update_args(&action)).unwrap_err();
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
    let err = super::build_update_params(as_update_args(&action)).unwrap_err();
    assert!(matches!(
        err,
        crate::error::BzrError::InputValidation(ref m) if m.contains("empty comment")
    ));
}

#[test]
fn build_update_params_rejects_comment_and_comment_file_together() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("body.txt");
    std::fs::write(&path, "from a file").unwrap();
    let action = make_update_action_with_comment(vec![1], Some("inline"), Some(&path), false);
    let err = super::build_update_params(as_update_args(&action)).unwrap_err();
    match err {
        crate::error::BzrError::InputValidation(msg) => {
            assert!(msg.contains("--comment"), "names inline flag: {msg}");
            assert!(msg.contains("--comment-file"), "names file flag: {msg}");
        }
        other => panic!("expected InputValidation, got {other:?}"),
    }
}

#[test]
fn build_update_params_reads_comment_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("body.txt");
    std::fs::write(&path, "from a file").unwrap();
    let action = make_update_action_with_comment(vec![1], None, Some(&path), false);
    let (_ids, params) = super::build_update_params(as_update_args(&action)).unwrap();
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
    let (_ids, params) = super::build_update_params(as_update_args(&action)).unwrap();
    let comment = params.comment.expect("comment populated");
    assert!(comment.is_private);
}

#[test]
fn build_update_params_rejects_missing_comment_file() {
    let path = std::path::Path::new("/nonexistent/bzr-issue-161-test.txt");
    let action = make_update_action_with_comment(vec![1], None, Some(path), false);
    let err = super::build_update_params(as_update_args(&action)).unwrap_err();
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
    let err = super::build_update_params(as_update_args(&action)).unwrap_err();
    assert!(matches!(err, crate::error::BzrError::InputValidation(_)));
}

#[test]
fn build_update_params_rejects_whitespace_only_comment_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("body.txt");
    std::fs::write(&path, "   \n\t  \n").unwrap();
    let action = make_update_action_with_comment(vec![1], None, Some(&path), false);
    let err = super::build_update_params(as_update_args(&action)).unwrap_err();
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
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Table, None),
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
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Table, None),
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
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Table, None),
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
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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

fn make_empty_update_action(ids: Vec<u64>) -> BugAction {
    let mut action = make_update_action(ids);
    if let BugAction::Update(crate::cli::UpdateArgs {
        status, resolution, ..
    }) = &mut action
    {
        *status = None;
        *resolution = None;
    }
    action
}

#[test]
fn build_update_params_rejects_update_with_no_fields() {
    let action = make_empty_update_action(vec![42]);
    let err = super::build_update_params(as_update_args(&action)).unwrap_err();
    assert!(
        matches!(err, crate::error::BzrError::InputValidation(ref msg) if msg.contains("at least one")),
        "expected an at-least-one-field validation error, got {err:?}"
    );
}

// ── Batch confirmation (#313) ───────────────────────────────────────

#[tokio::test]
async fn bug_update_large_batch_auto_proceeds_when_not_a_tty() {
    // Test stdin is not a TTY, so a >threshold batch must run without blocking
    // on a confirmation prompt — agents and pipes are never gated.
    let (_lock, mock, _tmp) = setup_test_env().await;
    let ids: Vec<u64> = (1..=11).collect();
    for &id in &ids {
        mock_put_bug_ok(&mock, id).await;
    }

    let action = make_update_action(ids.clone());
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "large batch should proceed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["action"], "updated");
    assert_eq!(parsed["succeeded"].as_array().unwrap().len(), 11);
}

// ── Optimistic concurrency (#320) ───────────────────────────────────

#[tokio::test]
async fn bug_update_expect_unchanged_proceeds_when_timestamp_matches() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_get_bug_lct(&mock, 42, "2026-06-19T12:00:00Z").await;
    mock_put_bug_ok(&mock, 42).await;

    let action = update_action_expect_unchanged(vec![42], "2026-06-19T12:00:00Z");
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    let output = io.out_str().to_string();

    assert!(
        result.is_ok(),
        "matching timestamp should update: {result:?}"
    );
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["action"], "updated");
    assert_eq!(parsed["id"], 42);
}

#[tokio::test]
async fn bug_update_expect_unchanged_matches_across_timestamp_formats() {
    // The server returns the XML-RPC basic form while the user passes the REST
    // canonical form of the same instant: the guard keys them equal and writes.
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_get_bug_lct(&mock, 42, "20260619T12:00:00").await;
    mock_put_bug_ok(&mock, 42).await;

    let action = update_action_expect_unchanged(vec![42], "2026-06-19T12:00:00Z");
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "cross-format timestamps should match and update: {result:?}"
    );
}

#[tokio::test]
async fn bug_update_expect_unchanged_detects_collision_and_skips_write() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_get_bug_lct(&mock, 42, "2026-06-19T12:00:00Z").await;
    forbid_put(&mock).await;

    // The bug last changed at 12:00 but we expected it unchanged since 10:00.
    let action = update_action_expect_unchanged(vec![42], "2026-06-19T10:00:00Z");
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::MidAirCollision { id: 42, .. }),
        "expected a collision on bug 42, got {err:?}"
    );
    assert_eq!(err.exit_code(), 14);
}

#[tokio::test]
async fn bug_update_expect_unchanged_batch_aborts_all_on_any_collision() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_get_bug_lct(&mock, 1, "2026-06-19T12:00:00Z").await; // matches
    mock_get_bug_lct(&mock, 2, "2026-06-19T13:00:00Z").await; // differs -> collision
    forbid_put(&mock).await;

    let action = update_action_expect_unchanged(vec![1, 2], "2026-06-19T12:00:00Z");
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    // Bug 2's collision aborts the whole batch before any write (forbid_put).
    assert!(matches!(
        result,
        Err(crate::error::BzrError::MidAirCollision { id: 2, .. })
    ));
}

#[tokio::test]
async fn bug_update_expect_unchanged_rejects_unparseable_expected_value() {
    // An unrecognized --expect-unchanged-since value fails before any re-read or
    // write: InputValidation (exit 7), and no GET/PUT is issued.
    let (_lock, mock, _tmp) = setup_test_env().await;
    forbid_put(&mock).await;

    let action = update_action_expect_unchanged(vec![42], "yesterday");
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation(_)),
        "unparseable expected value should be InputValidation, got {err:?}"
    );
    assert_eq!(err.exit_code(), 7);
}

#[tokio::test]
async fn bug_update_expect_unchanged_errors_when_server_omits_last_change_time() {
    // The re-read returns a bug with no last_change_time: the guard cannot
    // verify, so it fails with DataIntegrity (exit 10) and skips the write.
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": [{"id": 42}]})),
        )
        .mount(&mock)
        .await;
    forbid_put(&mock).await;

    let action = update_action_expect_unchanged(vec![42], "2026-06-19T12:00:00Z");
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::DataIntegrity(_)),
        "missing last_change_time should be DataIntegrity, got {err:?}"
    );
    assert_eq!(err.exit_code(), 10);
}

#[tokio::test]
async fn bug_update_expect_unchanged_errors_when_server_last_change_time_is_garbage() {
    // The re-read returns an unparseable last_change_time: the guard cannot key
    // it for comparison, so it fails with DataIntegrity (exit 10), not a false
    // collision and not a silent write.
    let (_lock, mock, _tmp) = setup_test_env().await;
    mock_get_bug_lct(&mock, 42, "not-a-timestamp").await;
    forbid_put(&mock).await;

    let action = update_action_expect_unchanged(vec![42], "2026-06-19T12:00:00Z");
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    let err = result.unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::DataIntegrity(_)),
        "garbage last_change_time should be DataIntegrity, got {err:?}"
    );
    assert_eq!(err.exit_code(), 10);
}

// ── Dry run (#308) ──────────────────────────────────────────────────

/// Mount a method-only PUT mock that must never fire — proves a dry run
/// performs no write. The connect-time TLS probe is a HEAD, so it won't match.
async fn forbid_put(mock: &wiremock::MockServer) {
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(mock)
        .await;
}

#[tokio::test]
async fn bug_update_dry_run_makes_no_write_and_marks_payload() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    forbid_put(&mock).await;

    let action = make_update_action(vec![42]);
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "dry-run update failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["resource"], "bug");
    assert_eq!(parsed["ids"], serde_json::json!([42]));
    assert_eq!(parsed["changes"]["status"], "RESOLVED");
    assert_eq!(parsed["changes"]["resolution"], "FIXED");
}

#[tokio::test]
async fn bug_update_dry_run_batch_lists_all_ids() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    forbid_put(&mock).await;

    let action = make_update_action(vec![1, 2, 3]);
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;
    let output = io.out_str().to_string();

    assert!(result.is_ok());
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([1, 2, 3]));
}

#[tokio::test]
async fn bug_update_dry_run_table_prints_human_preview() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    forbid_put(&mock).await;

    let action = make_update_action(vec![7, 8]);
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Table, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "dry-run update (table) failed: {result:?}");
    assert!(
        output.contains("Dry run") && output.contains("#7") && output.contains("#8"),
        "expected a human preview naming both bugs, got: {output:?}"
    );
}

#[tokio::test]
async fn bug_update_dry_run_still_validates_empty_update() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    forbid_put(&mock).await;

    let action = make_empty_update_action(vec![42]);
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;

    assert!(matches!(
        result,
        Err(crate::error::BzrError::InputValidation(_))
    ));
}
