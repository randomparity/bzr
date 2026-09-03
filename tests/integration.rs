//! Integration tests that exercise command dispatch end-to-end
//! with a wiremock server and real config file.
//!
//! These tests are serialized via a mutex because they set the
//! process-global `XDG_CONFIG_HOME` environment variable.

#![expect(clippy::unwrap_used, clippy::expect_used)]

use bzr::test_helpers::{setup_test_env, write_config_to, HasBooleanChartTriples};
use bzr::ENV_LOCK;

use clap::Parser;

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

#[test]
fn types_root_reexports_column_spec() {
    let spec = bzr::types::ColumnSpec::new(Some("id,status"), Some("status"));

    assert_eq!(spec.include, Some("id,status"));
    assert_eq!(spec.exclude, Some("status"));
}

// ── Bug commands ──────────────────────────────────────────────────────

#[tokio::test]
async fn bug_list_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [
                {"id": 1, "summary": "Test bug", "status": "NEW"}
            ]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&["bzr", "bug", "list"]).await;
    assert!(result.is_ok(), "bug list should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["id"], 1);
    assert_eq!(parsed[0]["summary"], "Test bug");
    assert_eq!(parsed[0]["status"], "NEW");
}

#[tokio::test]
async fn bug_list_changed_since_canonicalizes_bare_date_on_wire() {
    // End-to-end: a bare-date `--changed-since` value must be canonicalized
    // to `T00:00:00Z` by the time the REST request hits the server. Failing
    // this test means either the validator dropped the canonicalization
    // step or the encoder forgot to forward `last_change_time`.
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("product", "Firefox"))
        .and(query_param("last_change_time", "2026-04-01T00:00:00Z"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, _output) = dispatch_cli_with_output(&[
        "bzr",
        "bug",
        "list",
        "--product",
        "Firefox",
        "--changed-since",
        "2026-04-01",
    ])
    .await;
    assert!(
        result.is_ok(),
        "bug list with --changed-since should succeed: {result:?}"
    );
    // wiremock's `expect(1)` enforces that exactly one request matched the
    // canonicalized `last_change_time` query parameter; if the value were
    // sent as bare `2026-04-01`, the matcher would fail to match and the
    // response would be a 404, causing `result` to be `Err`.
}

#[tokio::test]
async fn bug_view_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 42, "summary": "Test bug", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&["bzr", "bug", "view", "42"]).await;
    assert!(result.is_ok(), "bug view should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["id"], 42);
    assert_eq!(parsed["summary"], "Test bug");
}

/// Red Hat Bugzilla serves `cc` as user objects (same shape as `cc_detail`)
/// to authenticated REST clients. bug view must deserialize that shape into
/// the string `cc` list instead of failing with exit 8.
#[tokio::test]
async fn bug_view_integration_cc_objects() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 42,
                "summary": "RH-shaped view",
                "status": "NEW",
                "cc": [
                    {"id": 215_372, "name": "airlied", "email": "airlied", "real_name": "Dave Airlie"},
                    {"id": 41342, "name": "bugproxy", "email": "bugproxy", "real_name": "IBM Bug Proxy"}
                ],
                "cc_detail": [
                    {"id": 215_372, "name": "airlied", "email": "airlied", "real_name": "Dave Airlie"},
                    {"id": 41342, "name": "bugproxy", "email": "bugproxy", "real_name": "IBM Bug Proxy"}
                ]
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&["bzr", "bug", "view", "42"]).await;
    assert!(
        result.is_ok(),
        "RH-shaped bug view should succeed: {result:?}"
    );
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["id"], 42);
    assert_eq!(
        parsed["cc"],
        serde_json::json!(["airlied", "bugproxy"]),
        "cc objects must collapse to login names"
    );
}

#[tokio::test]
async fn bug_search_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("quicksearch", "crash"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 99, "summary": "Crash on startup", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&["bzr", "bug", "search", "crash"]).await;
    assert!(result.is_ok(), "bug search should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["id"], 99);
    assert_eq!(parsed[0]["summary"], "Crash on startup");
}

#[tokio::test]
async fn bug_create_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 100})))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&[
        "bzr",
        "bug",
        "create",
        "--product",
        "TestProduct",
        "--component",
        "General",
        "--summary",
        "New bug",
        "--version",
        "unspecified",
        "--description",
        "body",
    ])
    .await;
    assert!(result.is_ok(), "bug create should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["id"], 100);
}

// ── Comment commands ──────────────────────────────────────────────────

#[tokio::test]
async fn comment_list_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": {
                    "comments": [
                        {"id": 1, "bug_id": 42, "text": "First comment", "count": 0}
                    ]
                }
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&["bzr", "comment", "list", "42"]).await;
    assert!(result.is_ok(), "comment list should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["id"], 1);
    assert_eq!(parsed[0]["text"], "First comment");
}

#[tokio::test]
async fn comment_add_body_file_posts_file_contents() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/7/comment"))
        .and(wiremock::matchers::body_string_contains("from a file"))
        .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"id": 99})))
        .expect(1)
        .mount(&mock)
        .await;

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("body.txt");
    std::fs::write(&file, "comment from a file\n").unwrap();

    let file_arg = file.to_str().unwrap();
    let result = dispatch_cli(&["bzr", "comment", "add", "7", "--body-file", file_arg]).await;
    assert!(
        result.is_ok(),
        "comment add --body-file should succeed: {result:?}"
    );
}

#[tokio::test]
async fn comment_add_body_and_body_file_conflict() {
    // clap conflicts_with surfaces as a parse error — no server needed.
    let parsed = bzr::cli::Cli::try_parse_from([
        "bzr",
        "comment",
        "add",
        "7",
        "--body",
        "x",
        "--body-file",
        "/tmp/x",
    ]);
    assert!(
        parsed.is_err(),
        "clap should reject --body with --body-file"
    );
}

// ── Whoami command ────────────────────────────────────────────────────

#[tokio::test]
async fn whoami_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1,
            "name": "admin@example.com",
            "real_name": "Admin User"
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&["bzr", "whoami"]).await;
    assert!(result.is_ok(), "whoami should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["name"], "admin@example.com");
    assert_eq!(parsed["real_name"], "Admin User");
}

// ── Product commands ──────────────────────────────────────────────────

#[tokio::test]
async fn product_list_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/product_accessible"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [1]})))
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{
                "id": 1, "name": "Firefox", "description": "Browser",
                "is_active": true, "components": [], "versions": [], "milestones": []
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&["bzr", "product", "list"]).await;
    assert!(result.is_ok(), "product list should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["name"], "Firefox");
}

// ── Server command ────────────────────────────────────────────────────

#[tokio::test]
async fn server_info_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/extensions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "extensions": {}
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&["bzr", "server", "info"]).await;
    assert!(result.is_ok(), "server info should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["version"], "5.1.2");
}

// ── Field command ─────────────────────────────────────────────────────

#[tokio::test]
async fn field_list_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug%5Fstatus"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{
                "name": "bug_status",
                "values": [
                    {"name": "NEW", "sort_key": 100, "is_active": true},
                    {"name": "RESOLVED", "sort_key": 500, "is_active": true}
                ]
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&["bzr", "field", "list", "status"]).await;
    assert!(result.is_ok(), "field list should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["name"], "NEW");
}

// ── Classification command ────────────────────────────────────────────

#[tokio::test]
async fn classification_view_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/classification/Unclassified"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "classifications": [{
                "id": 1,
                "name": "Unclassified",
                "description": "Default",
                "sort_key": 0,
                "products": []
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) =
        dispatch_cli_with_output(&["bzr", "classification", "view", "Unclassified"]).await;
    assert!(
        result.is_ok(),
        "classification view should succeed: {result:?}"
    );
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["name"], "Unclassified");
}

// ── User commands ─────────────────────────────────────────────────────

#[tokio::test]
async fn user_search_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{
                "id": 1,
                "name": "alice@example.com",
                "real_name": "Alice",
                "email": "alice@example.com",
                "groups": []
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&["bzr", "user", "search", "alice"]).await;
    assert!(result.is_ok(), "user search should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["name"], "alice@example.com");
    assert_eq!(parsed[0]["real_name"], "Alice");
}

// ── Group commands ────────────────────────────────────────────────────

#[tokio::test]
async fn group_view_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .and(query_param("names", "admin"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "groups": [{
                "id": 1,
                "name": "admin",
                "description": "Administrators",
                "is_active": true,
                "membership": []
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&["bzr", "group", "view", "admin"]).await;
    assert!(result.is_ok(), "group view should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["name"], "admin");
    assert_eq!(parsed["description"], "Administrators");
}

// ── Component commands ────────────────────────────────────────────────

#[tokio::test]
async fn component_create_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/component"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 10})))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&[
        "bzr",
        "component",
        "create",
        "--product",
        "TestProduct",
        "--name",
        "Backend",
        "--description",
        "Backend component",
        "--default-assignee",
        "dev@test.com",
    ])
    .await;
    assert!(
        result.is_ok(),
        "component create should succeed: {result:?}"
    );
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["id"], 10);
}

// ── Attachment commands ───────────────────────────────────────────────

#[tokio::test]
async fn attachment_list_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": [{
                    "id": 1,
                    "bug_id": 42,
                    "file_name": "patch.diff",
                    "summary": "Fix",
                    "content_type": "text/plain",
                    "size": 100
                }]
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&["bzr", "attachment", "list", "42"]).await;
    assert!(result.is_ok(), "attachment list should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["file_name"], "patch.diff");
}

// ── Config commands (no mock server needed) ───────────────────────────

#[tokio::test]
async fn config_show_integration() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::TempDir::new().unwrap();

    write_config_to(
        &tmp,
        r#"
default_server = "local"

[servers.local]
url = "https://bugzilla.local"
api_key = "key-1234567890"
"#,
    );
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let result = dispatch_cli(&["bzr", "config", "show"]).await;
    assert!(result.is_ok(), "config show should succeed: {result:?}");
}

// ── Error path: non-existent server ───────────────────────────────────

#[tokio::test]
async fn command_with_unknown_server_returns_error() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let result = dispatch_cli(&["bzr", "--server", "nonexistent", "bug", "list"]).await;
    assert!(result.is_err(), "should fail with unknown server");
}

// ── Error path: server returns API error ──────────────────────────────

#[tokio::test]
async fn api_error_propagates() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/99999"))
        .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({
            "error": true,
            "code": 101,
            "message": "Bug #99999 does not exist."
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&["bzr", "bug", "view", "99999"]).await;
    assert!(result.is_err(), "should propagate API error");
}

// ── Bug history ──────────────────────────────────────────────────────

#[tokio::test]
async fn bug_history_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/history"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 42,
                "history": [{
                    "when": "2025-01-01T00:00:00Z",
                    "who": "dev@example.com",
                    "changes": [{
                        "field_name": "status",
                        "removed": "NEW",
                        "added": "ASSIGNED"
                    }]
                }]
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;
    // The JSON path also fetches comments to correlate comment_id; none here.
    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": { "42": { "comments": [] } }
        })))
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&["bzr", "bug", "history", "42"]).await;
    assert!(result.is_ok(), "bug history should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    // Flattened change records (ADR 0008): one record per changed field.
    assert_eq!(parsed[0]["who"], "dev@example.com");
    assert_eq!(parsed[0]["when"], "2025-01-01T00:00:00Z");
    assert_eq!(parsed[0]["field"], "status");
    assert_eq!(parsed[0]["old_value"], "NEW");
    assert_eq!(parsed[0]["new_value"], "ASSIGNED");
    assert!(parsed[0]["comment_id"].is_null());
}

// ── Bug update ───────────────────────────────────────────────────────

#[tokio::test]
async fn bug_update_integration() {
    use wiremock::matchers::body_partial_json;

    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .and(body_partial_json(serde_json::json!({
            "keywords": {"add": ["fix-needed"], "remove": ["wontfix"]},
            "cc": {"add": ["alice@example.com"]},
            "groups": {"remove": ["secret"]},
            "see_also": {"add": ["https://example.com/issue/1"]},
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 42, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&[
        "bzr",
        "bug",
        "update",
        "42",
        "--status",
        "RESOLVED",
        "--resolution",
        "FIXED",
        "--keywords-add",
        "fix-needed",
        "--keywords-remove",
        "wontfix",
        "--cc-add",
        "alice@example.com",
        "--groups-remove",
        "secret",
        "--see-also-add",
        "https://example.com/issue/1",
    ])
    .await;
    assert!(result.is_ok(), "bug update should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["id"], 42);
    assert_eq!(parsed["action"], "updated");
}

#[tokio::test]
async fn bug_update_scalar_parity_fields_integration() {
    use wiremock::matchers::body_partial_json;

    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .and(body_partial_json(serde_json::json!({
            "alias": "short-name",
            "deadline": "2026-12-31",
            "estimated_time": 3.5,
            "remaining_time": 1.25,
            "work_time": 0.5,
            "url": "https://example.com/repro",
            "target_milestone": "5.0",
            "reset_assigned_to": true,
            "reset_qa_contact": true,
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 42, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "bug",
        "update",
        "42",
        "--alias",
        "short-name",
        "--deadline",
        "2026-12-31",
        "--estimated-time",
        "3.5",
        "--remaining-time",
        "1.25",
        "--work-time",
        "0.5",
        "--reset-assigned-to",
        "--reset-qa-contact",
        "--url",
        "https://example.com/repro",
        "--target-milestone",
        "5.0",
    ])
    .await;

    assert!(result.is_ok(), "bug update should succeed: {result:?}");
}

#[tokio::test]
async fn bug_update_with_comment_integration() {
    use wiremock::matchers::body_partial_json;

    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .and(body_partial_json(serde_json::json!({
            "status": "RESOLVED",
            "resolution": "FIXED",
            "comment": {
                "body": "see #other",
                "is_private": true,
            },
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 42, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "bug",
        "update",
        "42",
        "--status",
        "RESOLVED",
        "--resolution",
        "FIXED",
        "--comment",
        "see #other",
        "--comment-private",
    ])
    .await;
    assert!(
        result.is_ok(),
        "bug update with comment should succeed: {result:?}"
    );
}

// ── Comment add ──────────────────────────────────────────────────────

#[tokio::test]
async fn comment_add_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 999})))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&[
        "bzr",
        "comment",
        "add",
        "42",
        "--body",
        "This is a test comment",
    ])
    .await;
    assert!(result.is_ok(), "comment add should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["id"], 999);
}

// ── Comment tag ──────────────────────────────────────────────────────

#[tokio::test]
async fn comment_tag_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/bug/comment/100/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(["spam"])))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&["bzr", "comment", "tag", "100", "--add", "spam"]).await;
    assert!(result.is_ok(), "comment tag should succeed: {result:?}");
}

// ── Comment search tags ──────────────────────────────────────────────

#[tokio::test]
async fn comment_search_tags_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/comment/tags/spam"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(["spam"])))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&["bzr", "comment", "search-tags", "spam"]).await;
    assert!(
        result.is_ok(),
        "comment search-tags should succeed: {result:?}"
    );
}

// ── Attachment download ──────────────────────────────────────────────

#[tokio::test]
async fn attachment_download_integration() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/attachment/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": {
                "99": {
                    "id": 99,
                    "file_name": "test.txt",
                    "data": "SGVsbG8gd29ybGQ=",
                    "content_type": "text/plain",
                    "size": 11,
                    "summary": "Test file",
                    "bug_id": 42
                }
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let out_path = tmp.path().join("downloaded.txt");
    let out_arg = out_path.to_str().unwrap();
    let result = dispatch_cli(&["bzr", "attachment", "download", "99", "--out", out_arg]).await;
    assert!(
        result.is_ok(),
        "attachment download should succeed: {result:?}"
    );
    assert!(out_path.exists(), "downloaded file should exist");
    let content = std::fs::read_to_string(&out_path).unwrap();
    assert_eq!(content, "Hello world");
}

#[tokio::test]
async fn attachment_download_bulk_per_bug_integration() {
    let (_lock, mock, tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/77/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "77": [
                    {
                        "id": 1001,
                        "bug_id": 77,
                        "file_name": "a.txt",
                        "summary": "first",
                        "content_type": "text/plain",
                        "size": 5,
                        "is_obsolete": false,
                        "is_patch": false,
                        "is_private": false,
                        "data": "QUFBQUE="
                    },
                    {
                        "id": 1002,
                        "bug_id": 77,
                        "file_name": "b.txt",
                        "summary": "second",
                        "content_type": "text/plain",
                        "size": 4,
                        "is_obsolete": false,
                        "is_patch": false,
                        "is_private": false,
                        "data": "QkJCQg=="
                    }
                ]
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let out_dir = tmp.path().to_str().unwrap();
    let result = dispatch_cli(&[
        "bzr",
        "attachment",
        "download",
        "--bug",
        "77",
        "--out-dir",
        out_dir,
    ])
    .await;
    assert!(result.is_ok(), "expected ok: {result:?}");

    assert!(tmp.path().join("77").join("1001.a.txt").exists());
    assert!(tmp.path().join("77").join("1002.b.txt").exists());
    assert_eq!(
        std::fs::read(tmp.path().join("77").join("1001.a.txt")).unwrap(),
        b"AAAAA"
    );
    assert_eq!(
        std::fs::read(tmp.path().join("77").join("1002.b.txt")).unwrap(),
        b"BBBB"
    );
}

// ── Attachment upload ────────────────────────────────────────────────

#[tokio::test]
async fn attachment_upload_integration() {
    let (_lock, mock, tmp) = setup_test_env().await;

    // Create a temporary file to upload
    let upload_file = tmp.path().join("upload.txt");
    std::fs::write(&upload_file, "test content").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [101]})))
        .expect(1)
        .mount(&mock)
        .await;

    let file_arg = upload_file.to_str().unwrap();
    let result = dispatch_cli(&[
        "bzr",
        "attachment",
        "upload",
        "42",
        file_arg,
        "--summary",
        "Test upload",
        "--content-type",
        "text/plain",
    ])
    .await;
    assert!(
        result.is_ok(),
        "attachment upload should succeed: {result:?}"
    );
}

#[tokio::test]
async fn attachment_upload_with_comment_integration() {
    use wiremock::matchers::body_string_contains;
    let (_lock, mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("upload.txt");
    std::fs::write(&upload_file, "test content").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .and(body_string_contains(
            "\"comment\":\"see #6789 for context\"",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [102]})))
        .expect(1)
        .mount(&mock)
        .await;

    let file_arg = upload_file.to_str().unwrap();
    let result = dispatch_cli(&[
        "bzr",
        "attachment",
        "upload",
        "42",
        file_arg,
        "--summary",
        "Test upload",
        "--content-type",
        "text/plain",
        "--comment",
        "see #6789 for context",
    ])
    .await;
    assert!(
        result.is_ok(),
        "upload with --comment should succeed: {result:?}"
    );
}

#[tokio::test]
async fn attachment_upload_with_is_patch_integration() {
    use wiremock::matchers::body_string_contains;
    let (_lock, mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("fix.patch");
    std::fs::write(&upload_file, "diff --git a/x b/x").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .and(body_string_contains("\"is_patch\":true"))
        .and(body_string_contains("\"content_type\":\"text/plain\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [103]})))
        .expect(1)
        .mount(&mock)
        .await;

    let file_arg = upload_file.to_str().unwrap();
    let result = dispatch_cli(&[
        "bzr",
        "attachment",
        "upload",
        "42",
        file_arg,
        "--summary",
        "Test patch",
        "--patch",
    ])
    .await;
    assert!(
        result.is_ok(),
        "upload with --patch should succeed: {result:?}"
    );
}

#[tokio::test]
async fn attachment_upload_with_comment_private_integration() {
    use wiremock::matchers::body_string_contains;
    let (_lock, mock, tmp) = setup_test_env().await;

    let upload_file = tmp.path().join("upload.txt");
    std::fs::write(&upload_file, "test content").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .and(body_string_contains("\"comment\":\"sensitive\""))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [202]})))
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": {
                    "comments": [
                        {"id": 800, "bug_id": 42, "text": "sensitive", "attachment_id": 202}
                    ]
                }
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .and(body_string_contains("\"comment_is_private\""))
        .and(body_string_contains("\"800\":true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let file_arg = upload_file.to_str().unwrap();
    let result = dispatch_cli(&[
        "bzr",
        "attachment",
        "upload",
        "42",
        file_arg,
        "--summary",
        "test",
        "--content-type",
        "text/plain",
        "--comment",
        "sensitive",
        "--comment-private",
    ])
    .await;
    assert!(
        result.is_ok(),
        "upload --comment-private should drive POST→GET→PUT to success: {result:?}"
    );
}

#[tokio::test]
async fn attachment_list_returns_is_patch_field_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": [{
                    "id": 100,
                    "bug_id": 42,
                    "file_name": "fix.patch",
                    "summary": "patch",
                    "content_type": "text/plain",
                    "creation_time": "2026-05-06T00:00:00Z",
                    "is_obsolete": false,
                    "is_private": false,
                    "is_patch": true,
                    "size": 12
                }]
            }
        })))
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&["bzr", "attachment", "list", "42"]).await;
    assert!(result.is_ok(), "list should succeed: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["is_patch"], true);
}

// ── Attachment update ────────────────────────────────────────────────

#[tokio::test]
async fn attachment_update_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/bug/attachment/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": [{"id": 99, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "attachment",
        "update",
        "99",
        "--summary",
        "Updated summary",
    ])
    .await;
    assert!(
        result.is_ok(),
        "attachment update should succeed: {result:?}"
    );
}

// ── Product view ─────────────────────────────────────────────────────

#[tokio::test]
async fn product_view_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{
                "id": 1, "name": "Firefox", "description": "Browser",
                "is_active": true, "components": [], "versions": [], "milestones": []
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&["bzr", "product", "view", "Firefox"]).await;
    assert!(result.is_ok(), "product view should succeed: {result:?}");
}

// ── Product create ───────────────────────────────────────────────────

#[tokio::test]
async fn product_create_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 5})))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "product",
        "create",
        "--name",
        "NewProduct",
        "--description",
        "A new product",
        "--version",
        "1.0",
        "--is-open",
        "true",
    ])
    .await;
    assert!(result.is_ok(), "product create should succeed: {result:?}");
}

// ── Product update ───────────────────────────────────────────────────

#[tokio::test]
async fn product_update_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/product/Firefox"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{"id": 1, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "product",
        "update",
        "Firefox",
        "--description",
        "Updated description",
    ])
    .await;
    assert!(result.is_ok(), "product update should succeed: {result:?}");
}

// ── User create ──────────────────────────────────────────────────────

#[tokio::test]
async fn user_create_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 42})))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "user",
        "create",
        "--email",
        "new@example.com",
        "--full-name",
        "New User",
    ])
    .await;
    assert!(result.is_ok(), "user create should succeed: {result:?}");
}

// ── User update ──────────────────────────────────────────────────────

#[tokio::test]
async fn user_update_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/user/alice%40example%2Ecom"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{"id": 1, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "user",
        "update",
        "alice@example.com",
        "--real-name",
        "Alice Updated",
    ])
    .await;
    assert!(result.is_ok(), "user update should succeed: {result:?}");
}

// ── Group create ─────────────────────────────────────────────────────

#[tokio::test]
async fn group_create_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 10})))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "group",
        "create",
        "--name",
        "testers",
        "--description",
        "Tester group",
        "--is-active",
        "true",
    ])
    .await;
    assert!(result.is_ok(), "group create should succeed: {result:?}");
}

// ── Group update ─────────────────────────────────────────────────────

#[tokio::test]
async fn group_update_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/group/testers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 10, "changes": {}
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "group",
        "update",
        "testers",
        "--description",
        "Updated testers",
    ])
    .await;
    assert!(result.is_ok(), "group update should succeed: {result:?}");
}

// ── Group add user ───────────────────────────────────────────────────

#[tokio::test]
async fn group_add_user_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/user/alice%40example%2Ecom"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{"id": 1, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "group",
        "add-user",
        "--group",
        "admin",
        "--user",
        "alice@example.com",
    ])
    .await;
    assert!(result.is_ok(), "group add-user should succeed: {result:?}");
}

// ── Group remove user ────────────────────────────────────────────────

#[tokio::test]
async fn group_remove_user_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/user/alice%40example%2Ecom"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{"id": 1, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "group",
        "remove-user",
        "--group",
        "admin",
        "--user",
        "alice@example.com",
    ])
    .await;
    assert!(
        result.is_ok(),
        "group remove-user should succeed: {result:?}"
    );
}

// ── Group list users ─────────────────────────────────────────────────

#[tokio::test]
async fn group_list_users_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{
                "id": 1,
                "name": "alice@example.com",
                "real_name": "Alice",
                "email": "alice@example.com",
                "groups": [{"name": "admin"}]
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&["bzr", "group", "list-users", "--group", "admin"]).await;
    assert!(
        result.is_ok(),
        "group list-users should succeed: {result:?}"
    );
}

// ── Config set-server and set-default ────────────────────────────────

#[tokio::test]
async fn config_set_server_integration() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::TempDir::new().unwrap();

    write_config_to(
        &tmp,
        "default_server = \"local\"\n\n[servers.local]\nurl = \"https://bugzilla.local\"\napi_key = \"key-1234567890\"\n",
    );
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let result = dispatch_cli(&[
        "bzr",
        "config",
        "set-server",
        "staging",
        "--url",
        "https://staging.bugzilla.example",
        "--api-key",
        "staging-key-abc",
    ])
    .await;
    assert!(
        result.is_ok(),
        "config set-server should succeed: {result:?}"
    );
}

#[tokio::test]
async fn config_set_default_integration() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::TempDir::new().unwrap();

    write_config_to(
        &tmp,
        "default_server = \"local\"\n\n[servers.local]\nurl = \"https://bugzilla.local\"\napi_key = \"key-1234567890\"\n\n[servers.staging]\nurl = \"https://staging.example\"\napi_key = \"staging-key\"\n",
    );
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let result = dispatch_cli(&["bzr", "config", "set-default", "staging"]).await;
    assert!(
        result.is_ok(),
        "config set-default should succeed: {result:?}"
    );
}

// ── CLI-to-execute end-to-end tests ──────────────────────────────────
// These test the full path: CLI arg parsing → command dispatch → API call

/// Parse CLI args and dispatch to the matching command, exercising the same
/// path as `main.rs::run()`.
async fn dispatch_cli(args: &[&str]) -> bzr::error::Result<()> {
    let cli = bzr::cli::Cli::try_parse_from(args)
        .map_err(|e| bzr::error::BzrError::input(e.to_string()))?;

    let format = if cli.json {
        bzr::types::OutputFormat::Json
    } else {
        cli.output.unwrap_or(bzr::types::OutputFormat::Json)
    };

    let mut io = bzr::test_helpers::CapturedIo::new();
    bzr::dispatch(&cli, format, &mut io.writers()).await
}

/// Like [`dispatch_cli`] but returns the captured stdout alongside the result,
/// for tests that inspect the printed output.
async fn dispatch_cli_with_output(args: &[&str]) -> (bzr::error::Result<()>, String) {
    let cli = match bzr::cli::Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            return (
                Err(bzr::error::BzrError::input(e.to_string())),
                String::new(),
            );
        }
    };
    let format = if cli.json {
        bzr::types::OutputFormat::Json
    } else {
        cli.output.unwrap_or(bzr::types::OutputFormat::Json)
    };
    let mut io = bzr::test_helpers::CapturedIo::new();
    let result = bzr::dispatch(&cli, format, &mut io.writers()).await;
    (result, io.out_str().to_string())
}

/// Like [`dispatch_cli_with_output`] but also returns captured stderr, for
/// tests that assert on warnings emitted to stderr.
async fn dispatch_cli_with_io(args: &[&str]) -> (bzr::error::Result<()>, String, String) {
    let cli = match bzr::cli::Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            return (
                Err(bzr::error::BzrError::input(e.to_string())),
                String::new(),
                String::new(),
            );
        }
    };
    let format = if cli.json {
        bzr::types::OutputFormat::Json
    } else {
        cli.output.unwrap_or(bzr::types::OutputFormat::Json)
    };
    let mut io = bzr::test_helpers::CapturedIo::new();
    let result = bzr::dispatch(&cli, format, &mut io.writers()).await;
    (result, io.out_str().to_string(), io.err_str().to_string())
}

#[tokio::test]
async fn e2e_bug_list_via_cli_args() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1, "summary": "CLI test", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&[
        "bzr",
        "--server",
        "test",
        "--json",
        "bug",
        "list",
        "--product",
        "Firefox",
    ])
    .await;
    assert!(result.is_ok(), "e2e bug list: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed[0]["summary"], "CLI test");
}

#[tokio::test]
async fn e2e_bug_view_via_cli_args() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 42, "summary": "CLI view test", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) =
        dispatch_cli_with_output(&["bzr", "--server", "test", "--json", "bug", "view", "42"]).await;
    assert!(result.is_ok(), "e2e bug view: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["id"], 42);
    assert_eq!(parsed["summary"], "CLI view test");
}

#[tokio::test]
async fn e2e_whoami_via_cli_args() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 1,
            "name": "admin@example.com",
            "real_name": "Admin"
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, output) =
        dispatch_cli_with_output(&["bzr", "--server", "test", "--json", "whoami"]).await;
    assert!(result.is_ok(), "e2e whoami: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["name"], "admin@example.com");
}

/// #314 end-to-end: `bug view` against an inline `--server-url` server with no
/// config file on disk, driven through the real CLI parse + dispatch path.
#[tokio::test]
async fn e2e_inline_server_bug_view_without_config() {
    let _lock = ENV_LOCK.lock().await;
    let mock = wiremock::MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    // Empty XDG dir — no bzr/config.toml exists.
    // SAFETY: tests are serialized via ENV_LOCK.
    unsafe {
        std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        std::env::set_var("BZR_E2E_INLINE_KEY", "secret");
    }

    // Auth + version detection for the uncached inline server.
    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 1})))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.1.2"})),
        )
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 42, "summary": "Inline view", "status": "NEW"}]
        })))
        .mount(&mock)
        .await;

    let (result, output) = dispatch_cli_with_output(&[
        "bzr",
        "--server-url",
        &mock.uri(),
        "--server-api-key-env",
        "BZR_E2E_INLINE_KEY",
        "--json",
        "bug",
        "view",
        "42",
    ])
    .await;

    assert!(result.is_ok(), "inline e2e bug view: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["id"], 42);
    assert_eq!(parsed["summary"], "Inline view");
    assert!(
        !tmp.path().join("bzr").join("config.toml").exists(),
        "inline invocation must not write the config file"
    );
}

#[tokio::test]
async fn e2e_config_show_via_cli_args() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let result = dispatch_cli(&["bzr", "--json", "config", "show"]).await;
    assert!(result.is_ok(), "e2e config show: {result:?}");
}

#[tokio::test]
async fn e2e_skills_install_ignores_malformed_config_and_needs_no_server() {
    let _lock = ENV_LOCK.lock().await;
    let tmp = tempfile::TempDir::new().unwrap();
    let project = tmp.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let malformed_config = tmp.path().join("malformed-config.toml");
    let malformed_bytes = b"not = [valid toml\n";
    std::fs::write(&malformed_config, malformed_bytes).unwrap();

    let (result, output) = dispatch_cli_with_output(&[
        "bzr",
        "--config",
        malformed_config.to_str().unwrap(),
        "skills",
        "install",
        "--agent",
        "standard",
        "--project",
        project.to_str().unwrap(),
    ])
    .await;

    assert!(
        result.is_ok(),
        "local skill install should succeed: {result:?}"
    );
    assert!(
        malformed_config.exists(),
        "local skill install must not remove a Bugzilla config"
    );
    assert_eq!(
        std::fs::read(&malformed_config).unwrap(),
        malformed_bytes,
        "local skill install must not parse or rewrite malformed Bugzilla config"
    );
    assert!(
        project
            .join(".agents/skills/bzr-reference/reference/commands.md")
            .is_file(),
        "nested embedded payload should be installed"
    );
    let data = bzr::test_helpers::json_envelope_data(&output);
    assert_eq!(data["scope"], "project");
    assert_eq!(data["destinations"][0]["layout"], "agents");
}

#[tokio::test]
async fn e2e_server_info_via_cli_args() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "5.2"})),
        )
        .expect(1)
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/extensions"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"extensions": {}})),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&["bzr", "--server", "test", "--json", "server", "info"]).await;
    assert!(result.is_ok(), "e2e server info: {result:?}");
}

// ── Dispatch arm coverage: drives lib.rs::dispatch() for each remaining ──
// Commands::* arm not exercised by the e2e_*_via_cli_args tests above.
// Each test routes through bzr::dispatch() to ensure the match arm is hit.

#[tokio::test]
async fn e2e_comment_list_via_cli_args() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": {
                    "comments": [
                        {"id": 1, "bug_id": 42, "text": "First", "count": 0}
                    ]
                }
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result =
        dispatch_cli(&["bzr", "--server", "test", "--json", "comment", "list", "42"]).await;
    assert!(result.is_ok(), "e2e comment list: {result:?}");
}

#[tokio::test]
async fn e2e_attachment_list_via_cli_args() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": {
                "42": [{
                    "id": 1,
                    "bug_id": 42,
                    "file_name": "patch.diff",
                    "summary": "Fix",
                    "content_type": "text/plain",
                    "size": 100
                }]
            }
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "--server",
        "test",
        "--json",
        "attachment",
        "list",
        "42",
    ])
    .await;
    assert!(result.is_ok(), "e2e attachment list: {result:?}");
}

#[tokio::test]
async fn e2e_product_view_via_cli_args() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{
                "id": 1, "name": "Firefox", "description": "Browser",
                "is_active": true, "components": [], "versions": [], "milestones": []
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr", "--server", "test", "--json", "product", "view", "Firefox",
    ])
    .await;
    assert!(result.is_ok(), "e2e product view: {result:?}");
}

#[tokio::test]
async fn e2e_field_list_via_cli_args() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug%5Fstatus"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{
                "name": "bug_status",
                "values": [
                    {"name": "NEW", "sort_key": 100, "is_active": true}
                ]
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr", "--server", "test", "--json", "field", "list", "status",
    ])
    .await;
    assert!(result.is_ok(), "e2e field list: {result:?}");
}

#[tokio::test]
async fn e2e_user_search_via_cli_args() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{
                "id": 1,
                "name": "alice@example.com",
                "real_name": "Alice",
                "email": "alice@example.com",
                "groups": []
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr", "--server", "test", "--json", "user", "search", "alice",
    ])
    .await;
    assert!(result.is_ok(), "e2e user search: {result:?}");
}

#[tokio::test]
async fn e2e_group_view_via_cli_args() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/group"))
        .and(query_param("names", "admin"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "groups": [{
                "id": 1,
                "name": "admin",
                "description": "Administrators",
                "is_active": true,
                "membership": []
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr", "--server", "test", "--json", "group", "view", "admin",
    ])
    .await;
    assert!(result.is_ok(), "e2e group view: {result:?}");
}

#[tokio::test]
async fn e2e_classification_view_via_cli_args() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/classification/Unclassified"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "classifications": [{
                "id": 1,
                "name": "Unclassified",
                "description": "Default",
                "sort_key": 0,
                "products": []
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "--server",
        "test",
        "--json",
        "classification",
        "view",
        "Unclassified",
    ])
    .await;
    assert!(result.is_ok(), "e2e classification view: {result:?}");
}

#[tokio::test]
async fn e2e_component_create_via_cli_args() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/component"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 11})))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "--server",
        "test",
        "--json",
        "component",
        "create",
        "--product",
        "TestProduct",
        "--name",
        "Backend",
        "--description",
        "Backend component",
        "--default-assignee",
        "dev@test.com",
    ])
    .await;
    assert!(result.is_ok(), "e2e component create: {result:?}");
}

#[tokio::test]
async fn e2e_template_list_via_cli_args() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let result = dispatch_cli(&["bzr", "--json", "template", "list"]).await;
    assert!(result.is_ok(), "e2e template list: {result:?}");
}

#[tokio::test]
async fn e2e_query_list_via_cli_args() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let result = dispatch_cli(&["bzr", "--json", "query", "list"]).await;
    assert!(result.is_ok(), "e2e query list: {result:?}");
}

// ── CLI parsing: --version / --help exit paths ───────────────────────

#[test]
fn cli_version_flag_exits_with_display_version() {
    let result = bzr::cli::Cli::try_parse_from(["bzr", "--version"]);
    let Err(err) = result else {
        unreachable!("--version should not produce a parsed Cli");
    };
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayVersion);
}

#[test]
fn cli_help_flag_exits_with_display_help() {
    let result = bzr::cli::Cli::try_parse_from(["bzr", "--help"]);
    let Err(err) = result else {
        unreachable!("--help should not produce a parsed Cli");
    };
    assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
}

#[test]
fn cli_missing_subcommand_errors() {
    let result = bzr::cli::Cli::try_parse_from(["bzr"]);
    let Err(err) = result else {
        unreachable!("bzr without a subcommand should require one");
    };
    assert_eq!(
        err.kind(),
        clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    );
}

#[tokio::test]
async fn bug_list_issue_158_mixed_positive_and_negation_reaches_wire() {
    // End-to-end: --product P (positive), --resolution '!FIXED'
    // (notequals), --whiteboard '!wip' (notsubstring) all reach the
    // wire correctly. Exercises the full pipeline:
    // BugAction → SearchParams → REST encoder → wiremock.
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("product", "P"))
        .and(HasBooleanChartTriples::new(&[
            ("status_whiteboard", "notsubstring", "wip"),
            ("resolution", "notequals", "FIXED"),
        ]))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
        "bzr",
        "bug",
        "list",
        "--product",
        "P",
        "--whiteboard",
        "!wip",
        "--resolution",
        "!FIXED",
    ])
    .await;
    assert!(result.is_ok(), "bug list should succeed: {result:?}");
    // wiremock's `expect(1)` enforces that exactly one request matched
    // every matcher above; if any operator or value were wrong, the matcher
    // would miss and the response would be a 404, causing `result` to be `Err`.
}

/// Guards the #206 `--json` prose against misleading phrasings. `--json` now
/// trims output to the selected fields, but the older marketing-style
/// "trims the payload" wordings should not reappear in the CLI help or manual.
#[test]
fn cli_and_docs_avoid_misleading_trim_phrasing() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let forbidden = [
        "trims the payload",
        "trim the response payload",
        "trims JSON output",
        "JSON: fields to return",
        "JSON: fields to exclude",
    ];

    let mut files = vec![root.join("docs/bzr-cli.md")];
    for entry in std::fs::read_dir(root.join("src/cli")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }

    for file in &files {
        let content = std::fs::read_to_string(file).unwrap();
        for phrase in forbidden {
            assert!(
                !content.contains(phrase),
                "{}: remove misleading phrase {phrase:?}",
                file.display()
            );
        }
    }
}

fn push_files_with_extension(
    dir: &std::path::Path,
    extension: &str,
    files: &mut Vec<std::path::PathBuf>,
) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            push_files_with_extension(&path, extension, files);
        } else if path.extension().is_some_and(|e| e == extension) {
            files.push(path);
        }
    }
}

fn check_stale_flag_line(
    path: &std::path::Path,
    line_no: usize,
    line: &str,
    forbidden: &[(&str, &str)],
    findings: &mut Vec<String>,
) {
    for (old, replacement) in forbidden {
        if line.contains(old) {
            findings.push(format!(
                "{}:{line_no}: replace {old:?} with {replacement:?}",
                path.display()
            ));
        }
    }
}

#[test]
fn docs_and_help_comments_use_current_long_flags() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let forbidden = [
        ("--is-patch", "--patch"),
        ("--format json", "--json or --output json"),
        ("--ndjson", "--output ndjson"),
    ];
    let mut findings = Vec::new();

    let mut prose_files = vec![root.join("README.md"), root.join("docs/bzr-cli.md")];
    push_files_with_extension(&root.join("agent-skills"), "md", &mut prose_files);
    push_files_with_extension(&root.join("schemas"), "json", &mut prose_files);
    prose_files.sort();

    for path in prose_files {
        let content = std::fs::read_to_string(&path).unwrap();
        for (idx, line) in content.lines().enumerate() {
            check_stale_flag_line(&path, idx + 1, line, &forbidden, &mut findings);
        }
    }

    let mut source_files = Vec::new();
    push_files_with_extension(&root.join("src"), "rs", &mut source_files);
    source_files.sort();
    for path in source_files {
        let content = std::fs::read_to_string(&path).unwrap();
        for (idx, line) in content.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("///") || trimmed.starts_with("//!") {
                check_stale_flag_line(&path, idx + 1, line, &forbidden, &mut findings);
            }
        }
    }

    assert!(
        findings.is_empty(),
        "stale CLI flag references:\n{}",
        findings.join("\n")
    );
}

// ── #206 --json field trimming ───────────────────────────────────────

fn json_keys(value: &serde_json::Value) -> Vec<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

/// An all-unknown `--fields` value under `--json` exits 7 before any network
/// I/O — measured against the full field universe, not the table defaults.
#[tokio::test]
async fn e2e_bug_list_json_all_unknown_fields_exits_7() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let (result, _out, _err) = dispatch_cli_with_io(&[
        "bzr",
        "--server",
        "test",
        "--json",
        "bug",
        "list",
        "--product",
        "Firefox",
        "--fields",
        "not_a_field,also_not_a_field",
    ])
    .await;

    let err = result.unwrap_err();
    assert_eq!(
        err.exit_code(),
        7,
        "all-unknown --fields under --json exits 7"
    );
}

/// A partial-unknown selection warns once on stderr and projects the array to
/// the known fields.
#[tokio::test]
async fn e2e_bug_list_json_partial_unknown_warns_and_projects() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 7, "summary": "boom", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, out, err) = dispatch_cli_with_io(&[
        "bzr",
        "--server",
        "test",
        "--json",
        "bug",
        "list",
        "--product",
        "Firefox",
        "--fields",
        "summary,not_a_field",
    ])
    .await;

    assert!(result.is_ok(), "partial-unknown should succeed: {result:?}");
    assert!(
        err.contains("ignoring unknown field(s): not_a_field"),
        "stderr warning: {err:?}"
    );
    let parsed = bzr::test_helpers::json_envelope_data(&out);
    assert_eq!(
        json_keys(&parsed[0]),
        vec!["summary"],
        "projected to summary only:\n{out}"
    );
}

/// A custom `cf_*` field is a valid dynamic selection: it is requested from the
/// server and emitted when Bugzilla returns it.
#[tokio::test]
async fn e2e_bug_list_json_custom_field_is_emitted() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("include_fields", "id,cf_release"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 7, "summary": "boom", "cf_release": "9.6"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, out, err) = dispatch_cli_with_io(&[
        "bzr",
        "--server",
        "test",
        "--json",
        "bug",
        "list",
        "--product",
        "Firefox",
        "--fields",
        "cf_release",
    ])
    .await;

    assert!(
        result.is_ok(),
        "custom field selection should succeed: {result:?}"
    );
    assert!(err.is_empty(), "custom field should not warn: {err:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&out);
    assert_eq!(json_keys(&parsed[0]), vec!["cf_release"]);
    assert_eq!(parsed[0]["cf_release"], "9.6");
}

/// `bug view --json` with an all-unknown field stays lenient: exit 0, an empty
/// `{}` object, and a stderr warning so the typo isn't silent.
#[tokio::test]
async fn e2e_bug_view_json_all_unknown_is_lenient_with_warning() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 42, "summary": "view", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, out, err) = dispatch_cli_with_io(&[
        "bzr", "--server", "test", "--json", "bug", "view", "42", "--fields", "sumary",
    ])
    .await;

    assert!(result.is_ok(), "view stays lenient: {result:?}");
    assert!(
        err.contains("ignoring unknown field(s): sumary"),
        "stderr warning: {err:?}"
    );
    let parsed = bzr::test_helpers::json_envelope_data(&out);
    assert!(
        parsed.as_object().unwrap().is_empty(),
        "empty object for all-unknown view:\n{out}"
    );
}

/// Single-ID `bug view --json --fields` trims the bare object to the selected
/// fields.
#[tokio::test]
async fn e2e_bug_view_json_single_trims_object() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 42, "summary": "view", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, out, _err) = dispatch_cli_with_io(&[
        "bzr", "--server", "test", "--json", "bug", "view", "42", "--fields", "summary",
    ])
    .await;

    assert!(result.is_ok(), "single view: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&out);
    assert_eq!(
        json_keys(&parsed),
        vec!["summary"],
        "trimmed object:\n{out}"
    );
}

/// Multi-ID `bug view --json --fields summary` trims each entry in `bugs`
/// while the `{"bugs": [...], "failed": [...]}` wrapper stays intact.
#[tokio::test]
async fn e2e_multi_bug_view_json_trims_bugs_keeps_wrapper() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1, "summary": "alpha", "status": "NEW"}]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 2, "summary": "beta", "status": "NEW"}]
        })))
        .mount(&mock)
        .await;

    let (result, out, _err) = dispatch_cli_with_io(&[
        "bzr", "--server", "test", "--json", "bug", "view", "1", "2", "--fields", "summary",
    ])
    .await;

    assert!(result.is_ok(), "multi view: {result:?}");
    let parsed = bzr::test_helpers::json_envelope_data(&out);
    let bugs = parsed["bugs"].as_array().unwrap();
    assert_eq!(bugs.len(), 2);
    for b in bugs {
        assert_eq!(json_keys(b), vec!["summary"], "each bug trimmed to summary");
    }
    assert!(
        parsed["failed"].as_array().unwrap().is_empty(),
        "failed wrapper key present and empty"
    );
}

/// End-to-end `--exclude-fields id --json` contract: the CLI drops `id` from
/// the output object, yet the bug still deserializes because `force_id_fields`
/// keeps `id` on the wire (the lone `id` exclude collapses to `None`, so no
/// `exclude_fields` query param is sent). The mock is gated on
/// `query_param_is_missing("exclude_fields")`, so a regression that forwarded
/// `exclude_fields=id` would miss the matcher → 404 → this test fails.
#[tokio::test]
async fn e2e_bug_list_json_exclude_id_drops_key_but_parses() {
    use wiremock::matchers::query_param_is_missing;
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param_is_missing("exclude_fields"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1, "summary": "keep me", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let (result, out, _err) = dispatch_cli_with_io(&[
        "bzr",
        "--server",
        "test",
        "--json",
        "bug",
        "list",
        "--product",
        "Firefox",
        "--exclude-fields",
        "id",
    ])
    .await;

    assert!(
        result.is_ok(),
        "exclude id should parse and succeed: {result:?}"
    );
    let parsed = bzr::test_helpers::json_envelope_data(&out);
    let keys = json_keys(&parsed[0]);
    assert!(!keys.contains(&"id"), "id dropped from output:\n{out}");
    assert_eq!(parsed[0]["summary"], "keep me", "summary retained:\n{out}");
}

// ── Completion command ────────────────────────────────────────────────

/// `bzr completion <shell>` must parse and dispatch end-to-end (not just
/// the `commands::completion::execute` helper) and emit a non-empty script
/// naming the binary. Guards the `Commands::Completion` variant and the
/// `dispatch()` arm against silent removal/misrouting. Local-only: no
/// server, config, or env mutation, so it needs no `ENV_LOCK`.
#[tokio::test]
async fn completion_bash_parses_and_dispatches() {
    let cli = bzr::cli::Cli::try_parse_from(["bzr", "completion", "bash"])
        .expect("completion bash should parse");

    let mut __io = bzr::test_helpers::CapturedIo::new();
    let result = bzr::dispatch(&cli, bzr::types::OutputFormat::Table, &mut __io.writers()).await;
    assert!(
        result.is_ok(),
        "completion dispatch should succeed: {result:?}"
    );

    let script = __io.out_str();
    assert!(
        !script.is_empty(),
        "bash completion script should not be empty"
    );
    assert!(
        script.contains("bzr"),
        "bash completion script should name the bzr binary:\n{script}"
    );
    assert!(
        script.contains("complete"),
        "bash completion script should register a completion function:\n{script}"
    );
}

// ── JSON envelope invariant ──────────────────────────────────────────

/// Assert raw `--json` stdout is a single top-level object whose key set is
/// exactly `{schema_version, data}` with the current `schema_version` — guards
/// against a missing, doubled, or nested envelope from any command.
fn assert_single_envelope(raw: &str) {
    let parsed: serde_json::Value = serde_json::from_str(raw.trim()).unwrap();
    let obj = parsed.as_object().expect("not a top-level object");
    assert_eq!(
        json_keys(&parsed)
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        ["data", "schema_version"].into_iter().collect(),
        "envelope key set must be exactly {{schema_version, data}}:\n{raw}"
    );
    assert_eq!(
        obj["schema_version"].as_str().unwrap(),
        bzr::output::SCHEMA_VERSION
    );
}

/// Every `--json` family of output — a list, a single view, a mutation, and the
/// local `schema` list — carries exactly one versioned envelope.
#[tokio::test]
async fn json_output_carries_exactly_one_envelope() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1, "summary": "s", "status": "NEW"}]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("GET"))
        .and(path("/rest/bug/1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1, "summary": "s", "status": "NEW"}]
        })))
        .mount(&mock)
        .await;
    Mock::given(method("POST"))
        .and(path("/rest/bug/1/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 5})))
        .mount(&mock)
        .await;

    for args in [
        vec!["bzr", "bug", "list"],
        vec!["bzr", "bug", "view", "1"],
        vec!["bzr", "comment", "add", "1", "--body", "hi"],
        vec!["bzr", "schema"],
    ] {
        let (result, output) = dispatch_cli_with_output(&args).await;
        assert!(result.is_ok(), "{args:?} should succeed: {result:?}");
        assert_single_envelope(&output);
    }
}

/// An unknown shell name is rejected at parse time (clap value enum),
/// before any dispatch happens.
#[tokio::test]
async fn completion_rejects_unknown_shell() {
    let parsed = bzr::cli::Cli::try_parse_from(["bzr", "completion", "klingon"]);
    assert!(parsed.is_err(), "clap should reject an unknown shell name");
}

/// #323: the redundant `show` subcommand was removed. Bare `whoami` parses,
/// and `whoami show` is now rejected. (The parse routing to the `Whoami`
/// command variant is asserted in `src/cli/mod_tests.rs::parse_whoami`, which
/// can name the now crate-internal `Commands` type.)
#[tokio::test]
async fn whoami_bare_parses_and_show_subcommand_removed() {
    bzr::cli::Cli::try_parse_from(["bzr", "whoami"]).expect("bare whoami parses");

    let show = bzr::cli::Cli::try_parse_from(["bzr", "whoami", "show"]);
    assert!(show.is_err(), "`whoami show` should no longer be accepted");
}
