#![expect(clippy::unwrap_used)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{CommentAction, ProjectionArgs};
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

async fn mount_one_comment(mock: &wiremock::MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": { "42": { "comments": [{
                "id": 1, "bug_id": 42, "text": "Hello world",
                "creator": "user@test.com", "creation_time": "2025-01-01T00:00:00Z",
                "is_private": false, "count": 0
            }]}}
        })))
        .mount(mock)
        .await;
}

fn list_with(projection: ProjectionArgs) -> CommentAction {
    CommentAction::List {
        bug_ids: vec![42],
        permissive: false,
        since: None,
        projection,
    }
}

#[tokio::test]
async fn comment_list_returns_comments() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": {
                    "comments": [{
                        "id": 1,
                        "bug_id": 42,
                        "text": "Hello world",
                        "creator": "user@test.com",
                        "creation_time": "2025-01-01T00:00:00Z",
                        "is_private": false,
                        "count": 0
                    }]
                }
            }
        })))
        .mount(&mock)
        .await;

    let action = CommentAction::List {
        bug_ids: vec![42],
        permissive: false,
        since: None,
        projection: crate::cli::ProjectionArgs::default(),
    };
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a1.writers(),
    )
    .await;
    let output = __io_a1.out_str().to_string();
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["id"], 1);
    assert_eq!(parsed[0]["text"], "Hello world");
    assert_eq!(parsed[0]["creator"], "user@test.com");
}

#[tokio::test]
async fn comment_list_http_500_returns_error() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
        .mount(&mock)
        .await;

    let action = CommentAction::List {
        bug_ids: vec![42],
        permissive: false,
        since: None,
        projection: crate::cli::ProjectionArgs::default(),
    };
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("500") || err.contains("Internal Server Error"),
        "expected HTTP 500 error, got: {err}"
    );
}

#[tokio::test]
async fn comment_list_rejects_malformed_since_with_exit_code_7() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = CommentAction::List {
        bug_ids: vec![42],
        permissive: false,
        since: Some("nope".into()),
        projection: crate::cli::ProjectionArgs::default(),
    };
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 7);
    let msg = err.to_string();
    assert!(msg.contains("--since"), "error should name the flag: {msg}");
    assert!(
        msg.contains("nope"),
        "error should echo the offending input: {msg}"
    );
}

#[tokio::test]
async fn comment_list_json_fields_projects_to_named_keys() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_one_comment(&mock).await;

    let action = list_with(ProjectionArgs {
        fields: Some("id".into()),
        exclude_fields: None,
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    let parsed = crate::test_helpers::json_envelope_data(io.out_str());
    assert_eq!(parsed[0]["id"], 1);
    assert!(
        parsed[0].get("text").is_none(),
        "text should be projected out"
    );
    assert_eq!(parsed[0].as_object().unwrap().len(), 1);
}

#[tokio::test]
async fn comment_list_ndjson_fields_projects_each_line() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_one_comment(&mock).await;

    let action = list_with(ProjectionArgs {
        fields: Some("creator".into()),
        exclude_fields: None,
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(
            None,
            OutputFormat::Ndjson,
            None,
        ),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    assert_eq!(io.out_str().trim(), r#"{"creator":"user@test.com"}"#);
}

#[tokio::test]
async fn comment_list_json_unknown_field_exits_7() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = list_with(ProjectionArgs {
        fields: Some("creatorx".into()),
        exclude_fields: None,
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert_eq!(result.unwrap_err().exit_code(), 7);
    assert!(
        io.out_str().is_empty(),
        "nothing should be written on validation error"
    );
}

#[tokio::test]
async fn comment_list_table_fields_is_noop_with_warning() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_one_comment(&mock).await;

    let action = list_with(ProjectionArgs {
        fields: Some("id".into()),
        exclude_fields: None,
    });
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok());
    assert!(
        io.out_str().contains("Hello world"),
        "table body should still render"
    );
    assert!(
        io.err_str()
            .contains("--fields/--exclude-fields only affect"),
        "table mode should warn: {}",
        io.err_str()
    );
}

// ── Issue #699: multi-ID `comment list` ──────────────────────────────────

async fn mount_comment_for(mock: &wiremock::MockServer, bug: u64, text: &str) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/bug/{bug}/comment")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": { bug.to_string(): { "comments": [{
                "id": bug, "bug_id": bug, "text": text,
                "creator": "user@test.com",
                "creation_time": "2025-01-01T00:00:00Z",
                "is_private": false, "count": 0
            }]}}
        })))
        .mount(mock)
        .await;
}

/// A bug the server rejects as a per-resource failure. The body MUST be a
/// Bugzilla error object: a client-error status maps to `BzrError::Api { code }`
/// only when the body deserializes as an error response, and a plain-text body
/// falls through to `BzrError::HttpStatus`, which
/// `is_permissive_bug_view_error` rejects.
async fn mount_missing_bug(mock: &wiremock::MockServer, bug: u64) {
    Mock::given(method("GET"))
        .and(path(format!("/rest/bug/{bug}/comment")))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": true, "code": 101,
            "message": format!("Bug #{bug} does not exist.")
        })))
        .mount(mock)
        .await;
}

fn multi(bug_ids: Vec<u64>, permissive: bool) -> CommentAction {
    CommentAction::List {
        bug_ids,
        permissive,
        since: None,
        projection: ProjectionArgs::default(),
    }
}

/// `api` selects the transport. The handler builds its own client through
/// `connect_and_configure`, so a constructed `BugzillaClient` has nowhere to
/// go — `CommandContext::new`'s third argument is the only injection point, and
/// it overrides the `api_mode = "rest"` that `setup_test_env` writes.
async fn run_with_api(
    action: &CommentAction,
    format: OutputFormat,
    api: Option<crate::types::ApiMode>,
) -> (crate::error::Result<()>, String, String) {
    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::comment::execute(
        action,
        &crate::commands::runtime::invocation::CommandContext::new(None, format, api),
        &mut io.writers(),
    )
    .await;
    (result, io.out_str().to_string(), io.err_str().to_string())
}

async fn run(
    action: &CommentAction,
    format: OutputFormat,
) -> (crate::error::Result<()>, String, String) {
    run_with_api(action, format, None).await
}

#[tokio::test]
async fn multi_id_json_is_one_flat_array_in_argument_order() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_comment_for(&mock, 42, "first").await;
    mount_comment_for(&mock, 43, "second").await;

    let (result, out, _err) = run(&multi(vec![43, 42], false), OutputFormat::Json).await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&out);
    assert_eq!(parsed.as_array().unwrap().len(), 2);
    assert_eq!(parsed[0]["bug_id"], 43);
    assert_eq!(parsed[1]["bug_id"], 42);
}

#[tokio::test]
async fn single_id_json_stays_a_bare_array_with_no_header() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_comment_for(&mock, 42, "only").await;

    let (result, out, _err) = run(&multi(vec![42], false), OutputFormat::Json).await;
    assert!(result.is_ok());
    assert!(
        !out.contains("Bug #"),
        "single-ID JSON must not gain a header"
    );
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&out);
    assert_eq!(parsed.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn single_id_table_has_no_bug_header() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_comment_for(&mock, 42, "only").await;

    let (result, out, _err) = run(&multi(vec![42], false), OutputFormat::Table).await;
    assert!(result.is_ok());
    assert!(out.contains("only"));
    assert!(
        !out.contains("Bug #"),
        "single-ID table must not gain a header"
    );
}

/// Pins the single-ID output change named in the spec's Goal: `bug_id` moves
/// from `null` to the requested ID on a flat-envelope server.
#[tokio::test]
async fn single_id_flat_envelope_backfills_bug_id_in_json() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [{
                "id": 1, "text": "t", "creator": "u@t",
                "creation_time": "2025-01-01T00:00:00Z",
                "is_private": false, "count": 0
            }]
        })))
        .mount(&mock)
        .await;

    let (result, out, _err) = run(&multi(vec![42], false), OutputFormat::Json).await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&out);
    assert_eq!(parsed[0]["bug_id"], 42);
}

#[tokio::test]
async fn multi_id_table_writes_one_header_per_bug() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_comment_for(&mock, 42, "first").await;
    mount_comment_for(&mock, 43, "second").await;

    let (result, out, _err) = run(&multi(vec![42, 43], false), OutputFormat::Table).await;
    assert!(result.is_ok());
    assert!(out.contains("Bug #42"), "missing header for 42: {out}");
    assert!(out.contains("Bug #43"), "missing header for 43: {out}");
    assert!(
        out.find("Bug #42").unwrap() < out.find("Bug #43").unwrap(),
        "headers must follow argument order: {out}"
    );
}

#[tokio::test]
async fn permissive_skips_a_missing_bug_and_exits_zero() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_comment_for(&mock, 42, "first").await;
    mount_missing_bug(&mock, 99).await;

    let (result, out, err) = run(&multi(vec![42, 99], true), OutputFormat::Json).await;
    assert!(result.is_ok(), "permissive must not fail the call");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&out);
    assert_eq!(parsed.as_array().unwrap().len(), 1);
    assert_eq!(parsed[0]["bug_id"], 42);
    assert!(
        err.contains("99"),
        "stderr should name the failed bug: {err}"
    );
}

#[tokio::test]
async fn permissive_with_every_bug_failing_emits_an_empty_array() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_missing_bug(&mock, 98).await;
    mount_missing_bug(&mock, 99).await;

    let (result, out, err) = run(&multi(vec![98, 99], true), OutputFormat::Json).await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&out);
    assert!(parsed.as_array().unwrap().is_empty());
    assert!(err.contains("98") && err.contains("99"), "stderr: {err}");
}

/// NDJSON carries no envelope, so an empty result is byte-for-byte empty
/// stdout — there is no `data: []` marker for a consumer to read.
#[tokio::test]
async fn permissive_with_every_bug_failing_emits_empty_stdout_in_ndjson() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_missing_bug(&mock, 98).await;
    mount_missing_bug(&mock, 99).await;

    let (result, out, err) = run(&multi(vec![98, 99], true), OutputFormat::Ndjson).await;
    assert!(result.is_ok());
    assert!(
        out.is_empty(),
        "ndjson stdout should be empty, got: {out:?}"
    );
    assert!(err.contains("98") && err.contains("99"), "stderr: {err}");
}

#[tokio::test]
async fn permissive_with_every_bug_failing_prints_no_comments_in_table() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_missing_bug(&mock, 98).await;
    mount_missing_bug(&mock, 99).await;

    let (result, out, _err) = run(&multi(vec![98, 99], true), OutputFormat::Table).await;
    assert!(result.is_ok());
    assert_eq!(out.trim(), "No comments.");
    assert!(!out.contains("Bug #"));
}

/// `HttpStatus` is not per-resource, so `--permissive` must not swallow it.
#[tokio::test]
async fn permissive_still_aborts_on_a_plain_text_404() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_comment_for(&mock, 42, "first").await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/99/comment"))
        .respond_with(ResponseTemplate::new(404).set_body_string("not found"))
        .mount(&mock)
        .await;

    let (result, _out, _err) = run(&multi(vec![42, 99], true), OutputFormat::Json).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn without_permissive_a_missing_bug_aborts_with_no_partial_json() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_comment_for(&mock, 42, "first").await;
    mount_missing_bug(&mock, 99).await;

    let (result, out, _err) = run(&multi(vec![42, 99], false), OutputFormat::Json).await;
    assert!(result.is_err());
    assert!(out.is_empty(), "no partial JSON may be written: {out}");
}

#[tokio::test]
async fn permissive_with_one_id_exits_7_before_any_request() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let (result, out, _err) = run(&multi(vec![42], true), OutputFormat::Json).await;
    assert_eq!(result.unwrap_err().exit_code(), 7);
    assert!(out.is_empty());
}

#[tokio::test]
async fn multi_id_ndjson_emits_one_line_per_comment_across_bugs() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_comment_for(&mock, 42, "first").await;
    mount_comment_for(&mock, 43, "second").await;

    let (result, out, _err) = run(&multi(vec![42, 43], false), OutputFormat::Ndjson).await;
    assert!(result.is_ok());
    assert_eq!(out.trim().lines().count(), 2);
}

#[tokio::test]
async fn multi_id_fields_projection_keeps_bug_id() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_comment_for(&mock, 42, "first").await;
    mount_comment_for(&mock, 43, "second").await;

    let action = CommentAction::List {
        bug_ids: vec![42, 43],
        permissive: false,
        since: None,
        projection: ProjectionArgs {
            fields: Some("id".into()),
            exclude_fields: None,
        },
    };
    let (result, out, err) = run(&action, OutputFormat::Json).await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&out);
    for row in parsed.as_array().unwrap() {
        let obj = row.as_object().unwrap();
        assert_eq!(obj.len(), 2, "expected id + bug_id, got {obj:?}");
        assert!(obj.contains_key("id") && obj.contains_key("bug_id"));
    }
    assert!(err.contains("keeping bug_id"), "stderr: {err}");
}

#[tokio::test]
async fn multi_id_exclude_fields_cannot_drop_bug_id() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_comment_for(&mock, 42, "first").await;
    mount_comment_for(&mock, 43, "second").await;

    let action = CommentAction::List {
        bug_ids: vec![42, 43],
        permissive: false,
        since: None,
        projection: ProjectionArgs {
            fields: None,
            exclude_fields: Some("bug_id".into()),
        },
    };
    let (result, out, err) = run(&action, OutputFormat::Json).await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&out);
    for row in parsed.as_array().unwrap() {
        assert!(row.get("bug_id").is_some(), "bug_id was dropped: {row}");
    }
    assert!(err.contains("keeping bug_id"), "stderr: {err}");
}

/// The override is multi-ID only: one bug's comments need no attribution field.
#[tokio::test]
async fn single_id_fields_projection_is_not_overridden() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_comment_for(&mock, 42, "only").await;

    let action = CommentAction::List {
        bug_ids: vec![42],
        permissive: false,
        since: None,
        projection: ProjectionArgs {
            fields: Some("id".into()),
            exclude_fields: None,
        },
    };
    let (result, out, err) = run(&action, OutputFormat::Json).await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&out);
    assert_eq!(parsed[0].as_object().unwrap().len(), 1);
    assert!(!err.contains("keeping bug_id"), "stderr: {err}");
}

/// Exercises the `wrote_any = true` path after a *successful* empty fetch,
/// which is what distinguishes an empty thread from a skipped bug in table mode.
#[tokio::test]
async fn multi_id_bug_with_no_comments_gets_header_and_no_comments_line() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_comment_for(&mock, 42, "first").await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/43/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": { "43": { "comments": [] } }
        })))
        .mount(&mock)
        .await;

    let (result, out, _err) = run(&multi(vec![42, 43], false), OutputFormat::Table).await;
    assert!(result.is_ok());
    assert!(out.contains("Bug #43"), "missing header for 43: {out}");
    assert!(
        out.contains("No comments."),
        "missing empty-thread line: {out}"
    );
    assert!(
        out.find("Bug #43") < out.find("No comments."),
        "header must precede the line: {out}"
    );
}

#[tokio::test]
async fn multi_id_since_applies_to_every_bug() {
    use wiremock::matchers::query_param;

    let (_lock, mock, _tmp) = setup_test_env().await;
    for bug in [42_u64, 43] {
        Mock::given(method("GET"))
            .and(path(format!("/rest/bug/{bug}/comment")))
            // `parse_optional_date` canonicalizes a bare date to
            // `YYYY-MM-DDT00:00:00Z`, and that canonical form is what reaches
            // the wire.
            .and(query_param("new_since", "2025-01-01T00:00:00Z"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": { bug.to_string(): { "comments": [] } }
            })))
            .expect(1)
            .mount(&mock)
            .await;
    }

    let action = CommentAction::List {
        bug_ids: vec![42, 43],
        permissive: false,
        since: Some("2025-01-01".into()),
        projection: ProjectionArgs::default(),
    };
    let (result, _out, _err) = run(&action, OutputFormat::Json).await;
    assert!(result.is_ok());
    // Both `.expect(1)` mocks are verified when `mock` drops.
}

/// One `POST /xmlrpc.cgi` responder whose `Bug.comments` reply carries a `bugs`
/// struct keyed for 42 only, so bug 43's lookup finds no entry and Task 2's
/// `NotFound` fires.
async fn mount_xmlrpc_bug_42_only(mock: &wiremock::MockServer) {
    let body = r#"<?xml version="1.0"?>
<methodResponse><params><param><value><struct>
  <member><name>bugs</name><value><struct>
    <member><name>42</name><value><struct>
      <member><name>comments</name><value><array><data>
        <value><struct>
          <member><name>id</name><value><int>1</int></value></member>
          <member><name>bug_id</name><value><int>42</int></value></member>
          <member><name>count</name><value><int>0</int></value></member>
          <member><name>text</name><value><string>first</string></value></member>
          <member><name>creator</name><value><string>user@test.com</string></value></member>
          <member><name>creation_time</name><value><dateTime.iso8601>20250101T00:00:00</dateTime.iso8601></value></member>
          <member><name>is_private</name><value><boolean>0</boolean></value></member>
        </struct></value>
      </data></array></value></member>
    </struct></value></member>
  </struct></value></member>
</struct></value></param></params></methodResponse>"#;
    Mock::given(method("POST"))
        .and(path("/xmlrpc.cgi"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body, "text/xml"))
        .mount(mock)
        .await;
}

#[tokio::test]
async fn xmlrpc_multi_id_missing_bugs_key_aborts() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_xmlrpc_bug_42_only(&mock).await;

    let (result, _out, _err) = run_with_api(
        &multi(vec![42, 43], false),
        OutputFormat::Json,
        Some(crate::types::ApiMode::XmlRpc),
    )
    .await;
    let err = result.unwrap_err();
    assert!(
        err.is_permissive_bug_view_error(),
        "a dropped bug must be per-resource classifiable, got: {err:?}"
    );
}

#[tokio::test]
async fn xmlrpc_multi_id_missing_bugs_key_is_skipped_under_permissive() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_xmlrpc_bug_42_only(&mock).await;

    let (result, out, err) = run_with_api(
        &multi(vec![42, 43], true),
        OutputFormat::Json,
        Some(crate::types::ApiMode::XmlRpc),
    )
    .await;
    assert!(result.is_ok());
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&out);
    assert_eq!(parsed.as_array().unwrap().len(), 1);
    assert_eq!(parsed[0]["bug_id"], 42);
    assert!(
        err.contains("43"),
        "stderr should name the dropped bug: {err}"
    );
}
