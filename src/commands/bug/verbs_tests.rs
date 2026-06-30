#![expect(clippy::unwrap_used)]

use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{BugAction, CloseArgs, CommentArgs, DupArgs, ReopenArgs, ResolveArgs};
use crate::commands::runtime::invocation::inline_server::{InlineServer, InlineTlsOptions};
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn ok_put(id: u64) -> ResponseTemplate {
    ResponseTemplate::new(200)
        .set_body_json(serde_json::json!({"bugs": [{"id": id, "changes": {}}]}))
}

/// JSON body for a `GET field/bug/bug_status` response listing `statuses` as
/// the legal values. A leading null-named entry is included to mirror real
/// Bugzilla 5.0, which carries an unset/default entry the validator must skip.
fn status_field_body(statuses: &[&str]) -> serde_json::Value {
    let mut values = vec![serde_json::json!({"name": serde_json::Value::Null})];
    values.extend(statuses.iter().map(|s| serde_json::json!({"name": s})));
    serde_json::json!({"fields": [{"values": values}]})
}

/// Mount the `GET field/bug/bug_status` mock the close/reopen status validator
/// queries before writing.
async fn mount_status_field(mock: &wiremock::MockServer, statuses: &[&str]) {
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug_status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(status_field_body(statuses)))
        .mount(mock)
        .await;
}

async fn mount_inline_detection_mocks(mock: &wiremock::MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .expect(1)
        .mount(mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .expect(1)
        .mount(mock)
        .await;
}

/// Mount a PUT mock on `/rest/bug/{id}` asserting the exact JSON body, then run
/// `execute` for `action` and assert success.
async fn run_verb_expecting_body(action: BugAction, id: u64, body: serde_json::Value) {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("PUT"))
        .and(path(format!("/rest/bug/{id}")))
        .and(body_json(body))
        .respond_with(ok_put(id))
        .expect(1)
        .mount(&mock)
        .await;

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok(), "verb failed: {:?}", result.err());
}

/// Like [`run_verb_expecting_body`] but also mounts the status-field mock the
/// close/reopen validator queries. `statuses` are the server's legal statuses.
async fn run_status_verb(action: BugAction, id: u64, body: serde_json::Value, statuses: &[&str]) {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_status_field(&mock, statuses).await;
    Mock::given(method("PUT"))
        .and(path(format!("/rest/bug/{id}")))
        .and(body_json(body))
        .respond_with(ok_put(id))
        .expect(1)
        .mount(&mock)
        .await;

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok(), "verb failed: {:?}", result.err());
}

async fn run_verb_collision_expecting_no_write(action: BugAction, id: u64) {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_status_field(&mock, DEFAULT_STATUSES).await;
    Mock::given(method("GET"))
        .and(path(format!("/rest/bug/{id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": id, "last_change_time": "2026-06-19T12:05:00Z"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        matches!(result, Err(crate::error::BzrError::MidAirCollision { .. })),
        "expected mid-air collision, got {result:?}"
    );
}

const DEFAULT_STATUSES: &[&str] = &[
    "UNCONFIRMED",
    "CONFIRMED",
    "IN_PROGRESS",
    "RESOLVED",
    "VERIFIED",
];

fn close_args(ids: Vec<u64>, status: &str, as_resolution: Option<&str>) -> CloseArgs {
    CloseArgs {
        ids,
        status: status.into(),
        as_resolution: as_resolution.map(Into::into),
        expect_unchanged_since: None,
        comment: CommentArgs::default(),
    }
}

fn reopen_args(ids: Vec<u64>, status: &str) -> ReopenArgs {
    ReopenArgs {
        ids,
        status: status.into(),
        expect_unchanged_since: None,
        comment: CommentArgs::default(),
    }
}

#[tokio::test]
async fn resolve_dry_run_makes_no_write() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    // A verb PUT must never fire under --dry-run; the connect probe is a HEAD.
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let action = BugAction::Resolve(ResolveArgs {
        ids: vec![5],
        as_resolution: "FIXED".into(),
        expect_unchanged_since: None,
        comment: CommentArgs::default(),
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "dry-run resolve failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["ids"], serde_json::json!([5]));
    assert_eq!(parsed["changes"]["status"], "RESOLVED");
    assert_eq!(parsed["changes"]["resolution"], "FIXED");
}

#[tokio::test]
async fn resolve_defaults_to_fixed() {
    let action = BugAction::Resolve(ResolveArgs {
        ids: vec![5],
        as_resolution: "FIXED".into(),
        expect_unchanged_since: None,
        comment: CommentArgs::default(),
    });
    run_verb_expecting_body(
        action,
        5,
        serde_json::json!({"status": "RESOLVED", "resolution": "FIXED"}),
    )
    .await;
}

#[tokio::test]
async fn resolve_with_as_override() {
    let action = BugAction::Resolve(ResolveArgs {
        ids: vec![7],
        as_resolution: "WONTFIX".into(),
        expect_unchanged_since: None,
        comment: CommentArgs::default(),
    });
    run_verb_expecting_body(
        action,
        7,
        serde_json::json!({"status": "RESOLVED", "resolution": "WONTFIX"}),
    )
    .await;
}

#[tokio::test]
async fn bug_verbs_expect_unchanged_since_collision_skips_write() {
    let since = "2026-06-19T12:00:00Z";

    run_verb_collision_expecting_no_write(
        BugAction::Resolve(ResolveArgs {
            ids: vec![5],
            as_resolution: "FIXED".into(),
            expect_unchanged_since: Some(since.into()),
            comment: CommentArgs::default(),
        }),
        5,
    )
    .await;

    run_verb_collision_expecting_no_write(
        BugAction::Close(CloseArgs {
            ids: vec![6],
            status: "VERIFIED".into(),
            as_resolution: None,
            expect_unchanged_since: Some(since.into()),
            comment: CommentArgs::default(),
        }),
        6,
    )
    .await;

    run_verb_collision_expecting_no_write(
        BugAction::Reopen(ReopenArgs {
            ids: vec![7],
            status: "CONFIRMED".into(),
            expect_unchanged_since: Some(since.into()),
            comment: CommentArgs::default(),
        }),
        7,
    )
    .await;

    run_verb_collision_expecting_no_write(
        BugAction::Dup(DupArgs {
            id: 8,
            target: 80,
            expect_unchanged_since: Some(since.into()),
            comment: CommentArgs::default(),
        }),
        8,
    )
    .await;
}

#[tokio::test]
async fn bug_verbs_expect_unchanged_since_match_writes_update() {
    let since = "2026-06-19T12:00:00Z";
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 5, "last_change_time": since}]
        })))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/5"))
        .and(body_json(
            serde_json::json!({"status": "RESOLVED", "resolution": "FIXED"}),
        ))
        .respond_with(ok_put(5))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::Resolve(ResolveArgs {
        ids: vec![5],
        as_resolution: "FIXED".into(),
        expect_unchanged_since: Some(since.into()),
        comment: CommentArgs::default(),
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "guarded resolve failed: {result:?}");
}

#[tokio::test]
async fn close_defaults_to_verified_and_preserves_resolution() {
    let action = BugAction::Close(close_args(vec![9], "VERIFIED", None));
    // No resolution key — the server keeps any existing resolution.
    run_status_verb(
        action,
        9,
        serde_json::json!({"status": "VERIFIED"}),
        DEFAULT_STATUSES,
    )
    .await;
}

#[tokio::test]
async fn close_reuses_status_validation_client_for_update() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    // Inline servers are uncached. If close validates with one client and then
    // calls the generic update path that reconnects, auth/version detection
    // will fire twice and violate these expectations.
    mount_inline_detection_mocks(&mock).await;
    mount_status_field(&mock, DEFAULT_STATUSES).await;
    Mock::given(method("PUT"))
        .and(path("/rest/bug/9"))
        .and(body_json(serde_json::json!({"status": "VERIFIED"})))
        .respond_with(ok_put(9))
        .expect(1)
        .mount(&mock)
        .await;

    // SAFETY: setup_test_env holds ENV_LOCK for the duration of this test.
    unsafe { std::env::set_var("BZR_INLINE_TEST_KEY", "test-key") };
    let inline = InlineServer {
        url: mock.uri(),
        api_key_env: Some("BZR_INLINE_TEST_KEY".into()),
        email: None,
        tls: InlineTlsOptions::default(),
    };
    let action = BugAction::Close(close_args(vec![9], "VERIFIED", None));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_inline_server(Some(inline)),
        &mut io.writers(),
    )
    .await;

    assert!(result.is_ok(), "close failed: {result:?}");
}

#[tokio::test]
async fn close_with_as_sets_resolution() {
    let action = BugAction::Close(close_args(vec![9], "VERIFIED", Some("WONTFIX")));
    run_status_verb(
        action,
        9,
        serde_json::json!({"status": "VERIFIED", "resolution": "WONTFIX"}),
        DEFAULT_STATUSES,
    )
    .await;
}

#[tokio::test]
async fn close_status_override_targets_custom_status() {
    // An install that defines a custom CLOSED status reaches it via --status.
    let action = BugAction::Close(close_args(vec![9], "CLOSED", None));
    let mut statuses = DEFAULT_STATUSES.to_vec();
    statuses.push("CLOSED");
    run_status_verb(
        action,
        9,
        serde_json::json!({"status": "CLOSED"}),
        &statuses,
    )
    .await;
}

#[tokio::test]
async fn reopen_defaults_to_confirmed() {
    let action = BugAction::Reopen(reopen_args(vec![3], "CONFIRMED"));
    run_status_verb(
        action,
        3,
        serde_json::json!({"status": "CONFIRMED"}),
        DEFAULT_STATUSES,
    )
    .await;
}

#[tokio::test]
async fn reopen_status_override_targets_custom_status() {
    let action = BugAction::Reopen(reopen_args(vec![3], "REOPENED"));
    let mut statuses = DEFAULT_STATUSES.to_vec();
    statuses.push("REOPENED");
    run_status_verb(
        action,
        3,
        serde_json::json!({"status": "REOPENED"}),
        &statuses,
    )
    .await;
}

/// A status the server does not define is rejected client-side (exit 7) with a
/// message naming the bad value and listing valid statuses — and no PUT fires.
#[tokio::test]
async fn reopen_unknown_status_is_rejected() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_status_field(&mock, DEFAULT_STATUSES).await;
    Mock::given(method("PUT"))
        .respond_with(ok_put(3))
        .expect(0)
        .mount(&mock)
        .await;

    let action = BugAction::Reopen(reopen_args(vec![3], "REOPENED"));
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation { message: m, .. }
            if m.contains("REOPENED") && m.contains("CONFIRMED")),
        "got {err:?}"
    );
}

/// Matching is exact and case-sensitive: a wrong-case override is rejected up
/// front rather than passing validation and failing server-side.
#[tokio::test]
async fn close_wrong_case_status_is_rejected() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_status_field(&mock, DEFAULT_STATUSES).await;
    Mock::given(method("PUT"))
        .respond_with(ok_put(9))
        .expect(0)
        .mount(&mock)
        .await;

    let action = BugAction::Close(close_args(vec![9], "verified", None));
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation { message: m, .. } if m.contains("verified")),
        "got {err:?}"
    );
}

/// Under --dry-run no status-field GET and no PUT fire; the preview shows the
/// status that would be sent, even one this server would reject.
#[tokio::test]
async fn reopen_dry_run_skips_validation_and_write() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug_status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(status_field_body(DEFAULT_STATUSES)))
        .expect(0)
        .mount(&mock)
        .await;
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    // REOPENED is not in DEFAULT_STATUSES, but dry-run skips validation.
    let action = BugAction::Reopen(reopen_args(vec![3], "REOPENED"));
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await;
    let output = io.out_str().to_string();

    assert!(result.is_ok(), "dry-run reopen failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["action"], "dry-run");
    assert_eq!(parsed["changes"]["status"], "REOPENED");
}

#[tokio::test]
async fn close_dry_run_rejects_empty_status_without_network() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug_status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(status_field_body(DEFAULT_STATUSES)))
        .expect(0)
        .mount(&mock)
        .await;
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&mock)
        .await;

    let action = BugAction::Close(close_args(vec![9], " ", None));
    let mut io = crate::test_helpers::CapturedIo::new();
    let err = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None)
            .with_dry_run(true),
        &mut io.writers(),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(&err, crate::error::BzrError::InputValidation { message: m, .. }
            if m.contains("--status cannot be empty")),
        "got {err:?}"
    );
}

#[tokio::test]
async fn dup_sends_dupe_of() {
    let action = BugAction::Dup(DupArgs {
        id: 12,
        target: 100,
        expect_unchanged_since: None,
        comment: CommentArgs::default(),
    });
    run_verb_expecting_body(action, 12, serde_json::json!({"dupe_of": 100})).await;
}

#[tokio::test]
async fn resolve_posts_comment_atomically() {
    let action = BugAction::Resolve(ResolveArgs {
        ids: vec![5],
        as_resolution: "FIXED".into(),
        expect_unchanged_since: None,
        comment: CommentArgs {
            comment: Some("done in 9.1".into()),
            comment_file: None,
            comment_private: false,
        },
    });
    run_verb_expecting_body(
        action,
        5,
        // is_private is omitted when false (skip_serializing_if).
        serde_json::json!({
            "status": "RESOLVED",
            "resolution": "FIXED",
            "comment": {"body": "done in 9.1"}
        }),
    )
    .await;
}

#[tokio::test]
async fn close_posts_comment_atomically() {
    let action = BugAction::Close(CloseArgs {
        ids: vec![9],
        status: "VERIFIED".into(),
        as_resolution: None,
        expect_unchanged_since: None,
        comment: CommentArgs {
            comment: Some("closing note".into()),
            comment_file: None,
            comment_private: false,
        },
    });
    run_status_verb(
        action,
        9,
        serde_json::json!({"status": "VERIFIED", "comment": {"body": "closing note"}}),
        DEFAULT_STATUSES,
    )
    .await;
}

#[tokio::test]
async fn reopen_posts_comment_atomically() {
    let action = BugAction::Reopen(ReopenArgs {
        ids: vec![3],
        status: "CONFIRMED".into(),
        expect_unchanged_since: None,
        comment: CommentArgs {
            comment: Some("reopening".into()),
            comment_file: None,
            comment_private: false,
        },
    });
    run_status_verb(
        action,
        3,
        serde_json::json!({"status": "CONFIRMED", "comment": {"body": "reopening"}}),
        DEFAULT_STATUSES,
    )
    .await;
}

#[tokio::test]
async fn dup_posts_comment_atomically() {
    let action = BugAction::Dup(DupArgs {
        id: 12,
        target: 100,
        expect_unchanged_since: None,
        comment: CommentArgs {
            comment: Some("dupe of 100".into()),
            comment_file: None,
            comment_private: false,
        },
    });
    run_verb_expecting_body(
        action,
        12,
        serde_json::json!({"dupe_of": 100, "comment": {"body": "dupe of 100"}}),
    )
    .await;
}

#[tokio::test]
async fn resolve_batch_updates_each_id() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    for id in [1_u64, 2] {
        Mock::given(method("PUT"))
            .and(path(format!("/rest/bug/{id}")))
            .and(body_json(
                serde_json::json!({"status": "RESOLVED", "resolution": "FIXED"}),
            ))
            .respond_with(ok_put(id))
            .expect(1)
            .mount(&mock)
            .await;
    }

    let action = BugAction::Resolve(ResolveArgs {
        ids: vec![1, 2],
        as_resolution: "FIXED".into(),
        expect_unchanged_since: None,
        comment: CommentArgs::default(),
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok(), "batch resolve failed: {:?}", result.err());
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(io.out_str());
    // Batch JSON shape from update_batch / BatchResult.
    assert_eq!(parsed["succeeded"].as_array().unwrap().len(), 2);
}

/// The local comment-private validation fires before the network status check,
/// so no status-field GET is needed for this rejection.
#[tokio::test]
async fn close_private_comment_without_body_is_rejected() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let action = BugAction::Close(CloseArgs {
        ids: vec![5],
        status: "VERIFIED".into(),
        as_resolution: None,
        expect_unchanged_since: None,
        comment: CommentArgs {
            comment: None,
            comment_file: None,
            comment_private: true,
        },
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    let err = result.unwrap_err();
    assert!(
        matches!(&err, crate::error::BzrError::InputValidation { message: m, .. } if m.contains("--comment-private")),
        "got {err:?}"
    );
}
