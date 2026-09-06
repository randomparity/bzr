#![expect(clippy::unwrap_used)]
//! Shared `bug update` test fixtures: `BugAction::Update` builders and the
//! wiremock response mounts reused by the parent flow tests
//! (`update_tests.rs`) and the per-leaf sibling tests (`payload_tests.rs`,
//! `execute_tests.rs`). Helpers used by a single sibling stay local to that
//! sibling.

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::BugAction;

pub(super) fn make_update_action(ids: Vec<u64>) -> BugAction {
    BugAction::Update(crate::cli::UpdateArgs {
        ids,
        status: Some("RESOLVED".into()),
        resolution: Some("FIXED".into()),
        ..Default::default()
    })
}

pub(super) fn make_update_action_with_dupe_of(id: u64, dupe_of: u64) -> BugAction {
    BugAction::Update(crate::cli::UpdateArgs {
        ids: vec![id],
        dupe_of: Some(dupe_of),
        ..Default::default()
    })
}

pub(super) fn make_update_action_with_scalar_parity_fields() -> BugAction {
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

pub(super) fn make_update_action_with_comment(
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

pub(super) fn make_update_action_with_comment_tags(
    ids: Vec<u64>,
    comment: Option<&str>,
    comment_tag: Vec<&str>,
) -> BugAction {
    BugAction::Update(crate::cli::UpdateArgs {
        ids,
        comment: comment.map(String::from),
        comment_tag: comment_tag.into_iter().map(String::from).collect(),
        ..Default::default()
    })
}

pub(super) fn make_update_action_with_minor_update(ids: Vec<u64>, minor_update: bool) -> BugAction {
    BugAction::Update(crate::cli::UpdateArgs {
        ids,
        minor_update,
        ..Default::default()
    })
}

pub(super) fn make_empty_update_action(ids: Vec<u64>) -> BugAction {
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

pub(super) fn update_ids_mut(action: &mut BugAction) -> Option<&mut Vec<u64>> {
    let BugAction::Update(crate::cli::UpdateArgs { ids, .. }) = action else {
        return None;
    };
    Some(ids)
}

pub(super) async fn mock_put_bug_ok(mock: &wiremock::MockServer, id: u64) {
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
pub(super) async fn mock_get_bug_lct(mock: &wiremock::MockServer, id: u64, lct: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/bug/{id}")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"bugs": [{"id": id, "last_change_time": lct}]})),
        )
        .mount(mock)
        .await;
}

/// Mount a method-only PUT mock that must never fire — proves a path performs
/// no write. The connect-time TLS probe is a HEAD, so it won't match.
pub(super) async fn forbid_put(mock: &wiremock::MockServer) {
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(mock)
        .await;
}

pub(super) async fn received_put_count(mock: &wiremock::MockServer) -> usize {
    mock.received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|request| request.method.as_str() == "PUT")
        .count()
}
