//! Integration tests that exercise command dispatch end-to-end
//! with a wiremock server and real config file.
//!
//! These tests are serialized via a mutex because they set the
//! process-global `XDG_CONFIG_HOME` environment variable.

#![expect(clippy::unwrap_used, clippy::await_holding_lock)]

use std::sync::Mutex;

use clap::Parser;

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Serializes tests that modify `XDG_CONFIG_HOME`.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Writes a config file to a temp directory with the given server URL
/// and pre-cached auth method (Header) so auth detection is skipped.
fn setup_config(tmp: &tempfile::TempDir, server_url: &str) {
    let config_dir = tmp.path().join("bzr");
    std::fs::create_dir_all(&config_dir).unwrap();
    let config_content = format!(
        r#"
default_server = "test"

[servers.test]
url = "{server_url}"
api_key = "test-api-key-12345"
auth_method = "header"
api_mode = "rest"
"#,
    );
    std::fs::write(config_dir.join("config.toml"), config_content).unwrap();
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };
}

// ── Bug commands ──────────────────────────────────────────────────────

#[tokio::test]
async fn bug_list_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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

    let action = bzr::cli::BugAction::List {
        product: None,
        component: None,
        status: None,
        assignee: None,
        creator: None,
        priority: None,
        severity: None,
        id: vec![],
        alias: None,
        limit: 50,
        fields: None,
        exclude_fields: None,
    };
    let result =
        bzr::commands::bug::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "bug list should succeed: {result:?}");
    // Verify mock was called (wiremock panics on drop if expect(1) not satisfied)
}

#[tokio::test]
async fn bug_view_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 42, "summary": "Test bug", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::BugAction::View {
        id: "42".to_string(),
        fields: None,
        exclude_fields: None,
    };
    let result =
        bzr::commands::bug::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "bug view should succeed: {result:?}");
}

#[tokio::test]
async fn bug_search_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("quicksearch", "crash"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 99, "summary": "Crash on startup", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::BugAction::Search {
        query: "crash".to_string(),
        limit: 50,
        fields: None,
        exclude_fields: None,
    };
    let result =
        bzr::commands::bug::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "bug search should succeed: {result:?}");
}

#[tokio::test]
async fn bug_create_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("POST"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 100})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::BugAction::Create {
        product: "TestProduct".to_string(),
        component: "General".to_string(),
        summary: "New bug".to_string(),
        version: "unspecified".to_string(),
        description: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
    };
    let result =
        bzr::commands::bug::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "bug create should succeed: {result:?}");
}

// ── Comment commands ──────────────────────────────────────────────────

#[tokio::test]
async fn comment_list_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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

    let action = bzr::cli::CommentAction::List {
        bug_id: 42,
        since: None,
    };
    let result = bzr::commands::comment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(result.is_ok(), "comment list should succeed: {result:?}");
}

// ── Whoami command ────────────────────────────────────────────────────

#[tokio::test]
async fn whoami_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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

    let result =
        bzr::commands::whoami::execute(Some("test"), bzr::types::OutputFormat::Json, None).await;
    assert!(result.is_ok(), "whoami should succeed: {result:?}");
}

// ── Product commands ──────────────────────────────────────────────────

#[tokio::test]
async fn product_list_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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

    let action = bzr::cli::ProductAction::List {
        r#type: bzr::types::ProductListType::Accessible,
    };
    let result = bzr::commands::product::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(result.is_ok(), "product list should succeed: {result:?}");
}

// ── Server command ────────────────────────────────────────────────────

#[tokio::test]
async fn server_info_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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

    let action = bzr::cli::ServerAction::Info;
    let result =
        bzr::commands::server::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "server info should succeed: {result:?}");
}

// ── Field command ─────────────────────────────────────────────────────

#[tokio::test]
async fn field_list_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("GET"))
        .and(path("/rest/field/bug/status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{
                "values": [
                    {"name": "NEW", "sort_key": 100, "is_active": true},
                    {"name": "RESOLVED", "sort_key": 500, "is_active": true}
                ]
            }]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::FieldAction::List {
        name: "status".to_string(),
    };
    let result =
        bzr::commands::field::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "field list should succeed: {result:?}");
}

// ── Classification command ────────────────────────────────────────────

#[tokio::test]
async fn classification_view_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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

    let action = bzr::cli::ClassificationAction::View {
        name: "Unclassified".to_string(),
    };
    let result = bzr::commands::classification::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(
        result.is_ok(),
        "classification view should succeed: {result:?}"
    );
}

// ── User commands ─────────────────────────────────────────────────────

#[tokio::test]
async fn user_search_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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

    let action = bzr::cli::UserAction::Search {
        query: "alice".to_string(),
        details: false,
    };
    let result =
        bzr::commands::user::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "user search should succeed: {result:?}");
}

// ── Group commands ────────────────────────────────────────────────────

#[tokio::test]
async fn group_view_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("GET"))
        .and(path("/rest/group"))
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

    let action = bzr::cli::GroupAction::View {
        group: "admin".to_string(),
    };
    let result =
        bzr::commands::group::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "group view should succeed: {result:?}");
}

// ── Component commands ────────────────────────────────────────────────

#[tokio::test]
async fn component_create_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("POST"))
        .and(path("/rest/component"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 10})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::ComponentAction::Create {
        product: "TestProduct".to_string(),
        name: "Backend".to_string(),
        description: "Backend component".to_string(),
        default_assignee: "dev@test.com".to_string(),
    };
    let result = bzr::commands::component::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(
        result.is_ok(),
        "component create should succeed: {result:?}"
    );
}

// ── Attachment commands ───────────────────────────────────────────────

#[tokio::test]
async fn attachment_list_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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

    let action = bzr::cli::AttachmentAction::List { bug_id: 42 };
    let result = bzr::commands::attachment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(result.is_ok(), "attachment list should succeed: {result:?}");
}

// ── Config commands (no mock server needed) ───────────────────────────

#[test]
fn config_show_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::TempDir::new().unwrap();

    let config_dir = tmp.path().join("bzr");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        r#"
default_server = "local"

[servers.local]
url = "https://bugzilla.local"
api_key = "key-1234567890"
"#,
    )
    .unwrap();
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let action = bzr::cli::ConfigAction::Show;
    let result = bzr::commands::config_cmd::execute(&action, bzr::types::OutputFormat::Json);
    assert!(result.is_ok(), "config show should succeed: {result:?}");
}

// ── Error path: non-existent server ───────────────────────────────────

#[tokio::test]
async fn command_with_unknown_server_returns_error() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, "http://localhost:1");

    let action = bzr::cli::BugAction::List {
        product: None,
        component: None,
        status: None,
        assignee: None,
        creator: None,
        priority: None,
        severity: None,
        id: vec![],
        alias: None,
        limit: 50,
        fields: None,
        exclude_fields: None,
    };
    let result = bzr::commands::bug::execute(
        &action,
        Some("nonexistent"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(result.is_err(), "should fail with unknown server");
}

// ── Error path: server returns API error ──────────────────────────────

#[tokio::test]
async fn api_error_propagates() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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

    let action = bzr::cli::BugAction::View {
        id: "99999".to_string(),
        fields: None,
        exclude_fields: None,
    };
    let result =
        bzr::commands::bug::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_err(), "should propagate API error");
}

// ── Bug history ──────────────────────────────────────────────────────

#[tokio::test]
async fn bug_history_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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

    let action = bzr::cli::BugAction::History {
        id: 42,
        since: None,
    };
    let result =
        bzr::commands::bug::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "bug history should succeed: {result:?}");
}

// ── Bug update ───────────────────────────────────────────────────────

#[tokio::test]
async fn bug_update_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("PUT"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 42, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::BugAction::Update {
        id: 42,
        status: Some("RESOLVED".to_string()),
        resolution: Some("FIXED".to_string()),
        assignee: None,
        priority: None,
        severity: None,
        summary: None,
        whiteboard: None,
        flag: vec![],
    };
    let result =
        bzr::commands::bug::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "bug update should succeed: {result:?}");
}

// ── Comment add ──────────────────────────────────────────────────────

#[tokio::test]
async fn comment_add_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/comment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 999})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::CommentAction::Add {
        bug_id: 42,
        body: Some("This is a test comment".to_string()),
    };
    let result = bzr::commands::comment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(result.is_ok(), "comment add should succeed: {result:?}");
}

// ── Comment tag ──────────────────────────────────────────────────────

#[tokio::test]
async fn comment_tag_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("PUT"))
        .and(path("/rest/bug/comment/100/tags"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(["spam"])))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::CommentAction::Tag {
        comment_id: 100,
        add: vec!["spam".to_string()],
        remove: vec![],
    };
    let result = bzr::commands::comment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(result.is_ok(), "comment tag should succeed: {result:?}");
}

// ── Comment search tags ──────────────────────────────────────────────

#[tokio::test]
async fn comment_search_tags_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("GET"))
        .and(path("/rest/bug/comment/tags/spam"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(["spam"])))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::CommentAction::SearchTags {
        query: "spam".to_string(),
    };
    let result = bzr::commands::comment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(
        result.is_ok(),
        "comment search-tags should succeed: {result:?}"
    );
}

// ── Attachment download ──────────────────────────────────────────────

#[tokio::test]
async fn attachment_download_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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
    let action = bzr::cli::AttachmentAction::Download {
        id: 99,
        out: Some(out_path.to_string_lossy().into_owned()),
    };
    let result = bzr::commands::attachment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(
        result.is_ok(),
        "attachment download should succeed: {result:?}"
    );
    assert!(out_path.exists(), "downloaded file should exist");
}

// ── Attachment upload ────────────────────────────────────────────────

#[tokio::test]
async fn attachment_upload_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    // Create a temporary file to upload
    let upload_file = tmp.path().join("upload.txt");
    std::fs::write(&upload_file, "test content").unwrap();

    Mock::given(method("POST"))
        .and(path("/rest/bug/42/attachment"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"ids": [101]})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::AttachmentAction::Upload {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: Some("Test upload".to_string()),
        content_type: Some("text/plain".to_string()),
        flag: vec![],
    };
    let result = bzr::commands::attachment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(
        result.is_ok(),
        "attachment upload should succeed: {result:?}"
    );
}

// ── Attachment update ────────────────────────────────────────────────

#[tokio::test]
async fn attachment_update_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("PUT"))
        .and(path("/rest/bug/attachment/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": [{"id": 99, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::AttachmentAction::Update {
        id: 99,
        summary: Some("Updated summary".to_string()),
        file_name: None,
        content_type: None,
        obsolete: None,
        is_patch: None,
        is_private: None,
        flag: vec![],
    };
    let result = bzr::commands::attachment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(
        result.is_ok(),
        "attachment update should succeed: {result:?}"
    );
}

// ── Component update ─────────────────────────────────────────────────

#[tokio::test]
async fn component_update_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("PUT"))
        .and(path("/rest/component/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 10})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::ComponentAction::Update {
        id: 10,
        name: Some("Updated".to_string()),
        description: None,
        default_assignee: None,
    };
    let result = bzr::commands::component::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(
        result.is_ok(),
        "component update should succeed: {result:?}"
    );
}

// ── Product view ─────────────────────────────────────────────────────

#[tokio::test]
async fn product_view_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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

    let action = bzr::cli::ProductAction::View {
        name: "Firefox".to_string(),
    };
    let result = bzr::commands::product::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(result.is_ok(), "product view should succeed: {result:?}");
}

// ── Product create ───────────────────────────────────────────────────

#[tokio::test]
async fn product_create_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("POST"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 5})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::ProductAction::Create {
        name: "NewProduct".to_string(),
        description: "A new product".to_string(),
        version: "1.0".to_string(),
        is_open: true,
    };
    let result = bzr::commands::product::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(result.is_ok(), "product create should succeed: {result:?}");
}

// ── Product update ───────────────────────────────────────────────────

#[tokio::test]
async fn product_update_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("PUT"))
        .and(path("/rest/product/Firefox"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{"id": 1, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::ProductAction::Update {
        name: "Firefox".to_string(),
        description: Some("Updated description".to_string()),
        default_milestone: None,
        is_open: None,
    };
    let result = bzr::commands::product::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
    )
    .await;
    assert!(result.is_ok(), "product update should succeed: {result:?}");
}

// ── User create ──────────────────────────────────────────────────────

#[tokio::test]
async fn user_create_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("POST"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 42})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::UserAction::Create {
        email: "new@example.com".to_string(),
        full_name: Some("New User".to_string()),
        password: None,
    };
    let result =
        bzr::commands::user::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "user create should succeed: {result:?}");
}

// ── User update ──────────────────────────────────────────────────────

#[tokio::test]
async fn user_update_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("PUT"))
        .and(path("/rest/user/alice%40example%2Ecom"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{"id": 1, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::UserAction::Update {
        user: "alice@example.com".to_string(),
        real_name: Some("Alice Updated".to_string()),
        email: None,
        disable_login: None,
        login_denied_text: None,
    };
    let result =
        bzr::commands::user::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "user update should succeed: {result:?}");
}

// ── Group create ─────────────────────────────────────────────────────

#[tokio::test]
async fn group_create_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("POST"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 10})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::GroupAction::Create {
        name: "testers".to_string(),
        description: "Tester group".to_string(),
        is_active: true,
    };
    let result =
        bzr::commands::group::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "group create should succeed: {result:?}");
}

// ── Group update ─────────────────────────────────────────────────────

#[tokio::test]
async fn group_update_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("PUT"))
        .and(path("/rest/group/testers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 10, "changes": {}
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::GroupAction::Update {
        group: "testers".to_string(),
        description: Some("Updated testers".to_string()),
        is_active: None,
    };
    let result =
        bzr::commands::group::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "group update should succeed: {result:?}");
}

// ── Group add user ───────────────────────────────────────────────────

#[tokio::test]
async fn group_add_user_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("PUT"))
        .and(path("/rest/user/alice%40example%2Ecom"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{"id": 1, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::GroupAction::AddUser {
        group: "admin".to_string(),
        user: "alice@example.com".to_string(),
    };
    let result =
        bzr::commands::group::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(result.is_ok(), "group add-user should succeed: {result:?}");
}

// ── Group remove user ────────────────────────────────────────────────

#[tokio::test]
async fn group_remove_user_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("PUT"))
        .and(path("/rest/user/alice%40example%2Ecom"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{"id": 1, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::GroupAction::RemoveUser {
        group: "admin".to_string(),
        user: "alice@example.com".to_string(),
    };
    let result =
        bzr::commands::group::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(
        result.is_ok(),
        "group remove-user should succeed: {result:?}"
    );
}

// ── Group list users ─────────────────────────────────────────────────

#[tokio::test]
async fn group_list_users_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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

    let action = bzr::cli::GroupAction::ListUsers {
        group: "admin".to_string(),
        details: false,
    };
    let result =
        bzr::commands::group::execute(&action, Some("test"), bzr::types::OutputFormat::Json, None)
            .await;
    assert!(
        result.is_ok(),
        "group list-users should succeed: {result:?}"
    );
}

// ── Config set-server and set-default ────────────────────────────────

#[test]
fn config_set_server_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::TempDir::new().unwrap();

    let config_dir = tmp.path().join("bzr");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        "default_server = \"local\"\n\n[servers.local]\nurl = \"https://bugzilla.local\"\napi_key = \"key-1234567890\"\n",
    )
    .unwrap();
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let action = bzr::cli::ConfigAction::SetServer {
        name: "staging".to_string(),
        url: "https://staging.bugzilla.example".to_string(),
        api_key: "staging-key-abc".to_string(),
        email: None,
        auth_method: None,
    };
    let result = bzr::commands::config_cmd::execute(&action, bzr::types::OutputFormat::Json);
    assert!(
        result.is_ok(),
        "config set-server should succeed: {result:?}"
    );
}

#[test]
fn config_set_default_integration() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::TempDir::new().unwrap();

    let config_dir = tmp.path().join("bzr");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.toml"),
        "default_server = \"local\"\n\n[servers.local]\nurl = \"https://bugzilla.local\"\napi_key = \"key-1234567890\"\n\n[servers.staging]\nurl = \"https://staging.example\"\napi_key = \"staging-key\"\n",
    )
    .unwrap();
    unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path()) };

    let action = bzr::cli::ConfigAction::SetDefault {
        name: "staging".to_string(),
    };
    let result = bzr::commands::config_cmd::execute(&action, bzr::types::OutputFormat::Json);
    assert!(
        result.is_ok(),
        "config set-default should succeed: {result:?}"
    );
}

// ── CLI-to-execute end-to-end tests ──────────────────────────────────
// These test the full path: CLI arg parsing → command dispatch → API call

/// Parse CLI args and dispatch to the correct command `execute()` function,
/// exercising the same path as `main.rs::run()`.
async fn dispatch_cli(args: &[&str]) -> bzr::error::Result<()> {
    let cli = bzr::cli::Cli::try_parse_from(args)
        .map_err(|e| bzr::error::BzrError::InputValidation(e.to_string()))?;

    let format = if cli.json {
        bzr::types::OutputFormat::Json
    } else {
        cli.output.unwrap_or(bzr::types::OutputFormat::Json)
    };

    let api = cli.api;
    let server = cli.server.as_deref();

    match &cli.command {
        bzr::cli::Commands::Bug { action } => {
            bzr::commands::bug::execute(action, server, format, api).await
        }
        bzr::cli::Commands::Comment { action } => {
            bzr::commands::comment::execute(action, server, format, api).await
        }
        bzr::cli::Commands::Attachment { action } => {
            bzr::commands::attachment::execute(action, server, format, api).await
        }
        bzr::cli::Commands::Config { action } => bzr::commands::config_cmd::execute(action, format),
        bzr::cli::Commands::Product { action } => {
            bzr::commands::product::execute(action, server, format, api).await
        }
        bzr::cli::Commands::Field { action } => {
            bzr::commands::field::execute(action, server, format, api).await
        }
        bzr::cli::Commands::User { action } => {
            bzr::commands::user::execute(action, server, format, api).await
        }
        bzr::cli::Commands::Group { action } => {
            bzr::commands::group::execute(action, server, format, api).await
        }
        bzr::cli::Commands::Whoami => bzr::commands::whoami::execute(server, format, api).await,
        bzr::cli::Commands::Server { action } => {
            bzr::commands::server::execute(action, server, format, api).await
        }
        bzr::cli::Commands::Classification { action } => {
            bzr::commands::classification::execute(action, server, format, api).await
        }
        bzr::cli::Commands::Component { action } => {
            bzr::commands::component::execute(action, server, format, api).await
        }
    }
}

#[tokio::test]
async fn e2e_bug_list_via_cli_args() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1, "summary": "CLI test", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&[
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
}

#[tokio::test]
async fn e2e_bug_view_via_cli_args() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

    Mock::given(method("GET"))
        .and(path("/rest/bug/42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 42, "summary": "CLI view test", "status": "NEW"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let result = dispatch_cli(&["bzr", "--server", "test", "--json", "bug", "view", "42"]).await;
    assert!(result.is_ok(), "e2e bug view: {result:?}");
}

#[tokio::test]
async fn e2e_whoami_via_cli_args() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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

    let result = dispatch_cli(&["bzr", "--server", "test", "--json", "whoami"]).await;
    assert!(result.is_ok(), "e2e whoami: {result:?}");
}

#[tokio::test]
async fn e2e_config_show_via_cli_args() {
    let _lock = ENV_LOCK.lock().unwrap();
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, "http://localhost:1");

    let result = dispatch_cli(&["bzr", "--json", "config", "show"]).await;
    assert!(result.is_ok(), "e2e config show: {result:?}");
}

#[tokio::test]
async fn e2e_server_info_via_cli_args() {
    let _lock = ENV_LOCK.lock().unwrap();
    let mock = MockServer::start().await;
    let tmp = tempfile::TempDir::new().unwrap();
    setup_config(&tmp, &mock.uri());

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
