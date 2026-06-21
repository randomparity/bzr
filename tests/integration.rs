//! Integration tests that exercise command dispatch end-to-end
//! with a wiremock server and real config file.
//!
//! These tests are serialized via a mutex because they set the
//! process-global `XDG_CONFIG_HOME` environment variable.

#![expect(clippy::unwrap_used, clippy::expect_used)]

use bzr::test_helpers::setup_test_env;
use bzr::ENV_LOCK;

use clap::Parser;

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

/// Build a `BugAction::List` with every field at its default value.
/// Tests that exercise specific fields construct this and mutate the
/// fields they care about.
fn empty_list_action() -> bzr::cli::BugAction {
    bzr::cli::BugAction::List(bzr::cli::ListArgs {
        page_args: bzr::cli::PageArgs::default(),
        product: vec![],
        component: vec![],
        status: vec![],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        id: vec![],
        alias: None,
        summary: None,
        limit: 50,
        field_args: bzr::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        created_since: None,
        changed_since: None,
        whiteboard: vec![],
        target_milestone: vec![],
        version: vec![],
        op_sys: vec![],
        platform: vec![],
        resolution: vec![],
        qa_contact: vec![],
        url: vec![],
        sort_args: bzr::cli::SortArgs::default(),
        count: false,
    })
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

    let action = empty_list_action();
    let mut __io = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::bug::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io.writers(),
    )
    .await;
    let output = __io.out_str().to_string();
    assert!(result.is_ok(), "bug list should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let mut action = empty_list_action();
    if let bzr::cli::BugAction::List(bzr::cli::ListArgs {
        page_args: _,
        product,
        changed_since,
        ..
    }) = &mut action
    {
        *product = vec!["Firefox".into()];
        *changed_since = Some("2026-04-01".into());
    }
    let mut __io2 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::bug::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io2.writers(),
    )
    .await;
    let _output = __io2.out_str().to_string();
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

    let action = bzr::cli::BugAction::View(bzr::cli::ViewArgs {
        ids: vec!["42".to_string()],
        permissive: false,
        web: false,
        field_args: bzr::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
    });
    let mut __io3 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::bug::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io3.writers(),
    )
    .await;
    let output = __io3.out_str().to_string();
    assert!(result.is_ok(), "bug view should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["id"], 42);
    assert_eq!(parsed["summary"], "Test bug");
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

    let action = bzr::cli::BugAction::Search(bzr::cli::SearchArgs {
        page_args: bzr::cli::PageArgs::default(),
        query: Some("crash".to_string()),
        from_url: None,
        save_as: None,
        limit: None,
        field_args: bzr::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: bzr::cli::SortArgs::default(),
        count: false,
    });
    let mut __io4 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::bug::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io4.writers(),
    )
    .await;
    let output = __io4.out_str().to_string();
    assert!(result.is_ok(), "bug search should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let action = bzr::cli::BugAction::Create(bzr::cli::CreateArgs {
        from_json: None,
        template: None,
        product: Some("TestProduct".to_string()),
        component: Some("General".to_string()),
        summary: Some("New bug".to_string()),
        version: Some("unspecified".to_string()),
        description: Some("body".to_string()),
        description_file: None,
        priority: None,
        severity: None,
        assignee: None,
        op_sys: None,
        rep_platform: None,
        blocks: vec![],
        depends_on: vec![],
        create_fields: bzr::cli::CreateFieldArgs::default(),
    });
    let mut __io5 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::bug::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io5.writers(),
    )
    .await;
    let output = __io5.out_str().to_string();
    assert!(result.is_ok(), "bug create should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let action = bzr::cli::CommentAction::List {
        bug_id: 42,
        since: None,
    };
    let mut __io6 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::comment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io6.writers(),
    )
    .await;
    let output = __io6.out_str().to_string();
    assert!(result.is_ok(), "comment list should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let action = bzr::cli::CommentAction::Add {
        bug_id: 7,
        body: None,
        body_file: Some(file),
        private: false,
    };
    let mut __io_bf = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::comment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io_bf.writers(),
    )
    .await;
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

    let mut __io7 = bzr::test_helpers::CapturedIo::new();

    let result = bzr::commands::whoami::execute(
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io7.writers(),
    )
    .await;

    let output = __io7.out_str().to_string();
    assert!(result.is_ok(), "whoami should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let action = bzr::cli::ProductAction::List {
        r#type: bzr::types::ProductListType::Accessible,
    };
    let mut __io8 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::product::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io8.writers(),
    )
    .await;
    let output = __io8.out_str().to_string();
    assert!(result.is_ok(), "product list should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let action = bzr::cli::ServerAction::Info;
    let mut __io9 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::server::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io9.writers(),
    )
    .await;
    let output = __io9.out_str().to_string();
    assert!(result.is_ok(), "server info should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["version"], "5.1.2");
}

// ── Field command ─────────────────────────────────────────────────────

#[tokio::test]
async fn field_list_integration() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/field/bug/bug_status"))
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
    let mut __io10 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::field::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io10.writers(),
    )
    .await;
    let output = __io10.out_str().to_string();
    assert!(result.is_ok(), "field list should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let action = bzr::cli::ClassificationAction::View {
        name: "Unclassified".to_string(),
    };
    let mut __io11 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::classification::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io11.writers(),
    )
    .await;
    let output = __io11.out_str().to_string();
    assert!(
        result.is_ok(),
        "classification view should succeed: {result:?}"
    );
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let action = bzr::cli::UserAction::Search {
        query: "alice".to_string(),
        details: false,
    };
    let mut __io12 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::user::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io12.writers(),
    )
    .await;
    let output = __io12.out_str().to_string();
    assert!(result.is_ok(), "user search should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let action = bzr::cli::GroupAction::View {
        group: "admin".to_string(),
    };
    let mut __io13 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::group::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io13.writers(),
    )
    .await;
    let output = __io13.out_str().to_string();
    assert!(result.is_ok(), "group view should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let action = bzr::cli::ComponentAction::Create {
        from_json: None,
        product: Some("TestProduct".to_string()),
        name: Some("Backend".to_string()),
        description: Some("Backend component".to_string()),
        default_assignee: Some("dev@test.com".to_string()),
    };
    let mut __io14 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::component::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io14.writers(),
    )
    .await;
    let output = __io14.out_str().to_string();
    assert!(
        result.is_ok(),
        "component create should succeed: {result:?}"
    );
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let action = bzr::cli::AttachmentAction::List { bug_id: 42 };
    let mut __io15 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::attachment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io15.writers(),
    )
    .await;
    let output = __io15.out_str().to_string();
    assert!(result.is_ok(), "attachment list should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed[0]["file_name"], "patch.diff");
}

// ── Config commands (no mock server needed) ───────────────────────────

#[tokio::test]
async fn config_show_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let _lock = ENV_LOCK.lock().await;
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
    let result = bzr::commands::config::execute(
        &action,
        None,
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "config show should succeed: {result:?}");
}

// ── Error path: non-existent server ───────────────────────────────────

#[tokio::test]
async fn command_with_unknown_server_returns_error() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = empty_list_action();
    let result = bzr::commands::bug::execute(
        &action,
        Some("nonexistent"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "should fail with unknown server");
}

// ── Error path: server returns API error ──────────────────────────────

#[tokio::test]
async fn api_error_propagates() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
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

    let action = bzr::cli::BugAction::View(bzr::cli::ViewArgs {
        ids: vec!["99999".to_string()],
        permissive: false,
        web: false,
        field_args: bzr::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
    });
    let result = bzr::commands::bug::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
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

    let action = bzr::cli::BugAction::History(bzr::cli::HistoryArgs {
        id: 42,
        since: None,
    });
    let mut __io16 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::bug::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io16.writers(),
    )
    .await;
    let output = __io16.out_str().to_string();
    assert!(result.is_ok(), "bug history should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed[0]["who"], "dev@example.com");
    assert_eq!(parsed[0]["changes"][0]["field_name"], "status");
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

    let action = bzr::cli::BugAction::Update(bzr::cli::UpdateArgs {
        from_json: None,
        ids: vec![42],
        status: Some("RESOLVED".to_string()),
        resolution: Some("FIXED".to_string()),
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
        url: None,
        target_milestone: None,
        flag: vec![],
        blocks_add: vec![],
        blocks_remove: vec![],
        depends_on_add: vec![],
        depends_on_remove: vec![],
        keywords_add: vec!["fix-needed".to_string()],
        keywords_remove: vec!["wontfix".to_string()],
        cc_add: vec!["alice@example.com".to_string()],
        cc_remove: vec![],
        groups_add: vec![],
        groups_remove: vec!["secret".to_string()],
        see_also_add: vec!["https://example.com/issue/1".to_string()],
        see_also_remove: vec![],
        comment: None,
        comment_file: None,
        comment_private: false,
        expect_unchanged_since: None,
    });
    let mut __io17 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::bug::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io17.writers(),
    )
    .await;
    let output = __io17.out_str().to_string();
    assert!(result.is_ok(), "bug update should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let action = bzr::cli::BugAction::Update(bzr::cli::UpdateArgs {
        from_json: None,
        ids: vec![42],
        status: None,
        resolution: None,
        dupe_of: None,
        alias: Some("short-name".to_string()),
        deadline: Some("2026-12-31".to_string()),
        estimated_time: Some(3.5),
        remaining_time: Some(1.25),
        work_time: Some(0.5),
        reset_assigned_to: true,
        reset_qa_contact: true,
        assignee: None,
        priority: None,
        severity: None,
        summary: None,
        whiteboard: None,
        url: Some("https://example.com/repro".to_string()),
        target_milestone: Some("5.0".to_string()),
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
        expect_unchanged_since: None,
    });

    let mut io = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::bug::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut io.writers(),
    )
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

    let action = bzr::cli::BugAction::Update(bzr::cli::UpdateArgs {
        from_json: None,
        ids: vec![42],
        status: Some("RESOLVED".to_string()),
        resolution: Some("FIXED".to_string()),
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
        url: None,
        target_milestone: None,
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
        comment: Some("see #other".to_string()),
        comment_file: None,
        comment_private: true,
        expect_unchanged_since: None,
    });
    let mut __io18 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::bug::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io18.writers(),
    )
    .await;
    let _output = __io18.out_str().to_string();
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

    let action = bzr::cli::CommentAction::Add {
        bug_id: 42,
        body: Some("This is a test comment".to_string()),
        body_file: None,
        private: false,
    };
    let mut __io19 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::comment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io19.writers(),
    )
    .await;
    let output = __io19.out_str().to_string();
    assert!(result.is_ok(), "comment add should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["id"], 999);
}

// ── Comment tag ──────────────────────────────────────────────────────

#[tokio::test]
async fn comment_tag_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

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
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "comment tag should succeed: {result:?}");
}

// ── Comment search tags ──────────────────────────────────────────────

#[tokio::test]
async fn comment_search_tags_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

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
        &mut __cap_io.writers(),
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
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
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
    let action = bzr::cli::AttachmentAction::Download {
        ids: vec![99],
        bug_ids: vec![],
        out: Some(out_path.to_string_lossy().into_owned()),
        out_dir: "./attachments".into(),
    };
    let result = bzr::commands::attachment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
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
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
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

    let out_dir = tmp.path().to_string_lossy().into_owned();
    let action = bzr::cli::AttachmentAction::Download {
        ids: vec![],
        bug_ids: vec![77],
        out: None,
        out_dir,
    };
    let result = bzr::commands::attachment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
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
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
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

    let action = bzr::cli::AttachmentAction::Upload(bzr::cli::attachment::UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: Some("Test upload".to_string()),
        content_type: Some("text/plain".to_string()),
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: None,
        comment_private: false,
        flag: vec![],
    });
    let result = bzr::commands::attachment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "attachment upload should succeed: {result:?}"
    );
}

#[tokio::test]
async fn attachment_upload_with_comment_integration() {
    use wiremock::matchers::body_string_contains;
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
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

    let action = bzr::cli::AttachmentAction::Upload(bzr::cli::attachment::UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: Some("Test upload".to_string()),
        content_type: Some("text/plain".to_string()),
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: Some("see #6789 for context".to_string()),
        comment_private: false,
        flag: vec![],
    });
    let result = bzr::commands::attachment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "upload with --comment should succeed: {result:?}"
    );
}

#[tokio::test]
async fn attachment_upload_with_is_patch_integration() {
    use wiremock::matchers::body_string_contains;
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
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

    let action = bzr::cli::AttachmentAction::Upload(bzr::cli::attachment::UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: Some("Test patch".to_string()),
        content_type: None,
        private: false,
        no_private: false,
        patch: true,
        no_patch: false,
        comment: None,
        comment_private: false,
        flag: vec![],
    });
    let result = bzr::commands::attachment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "upload with --is-patch should succeed: {result:?}"
    );
}

#[tokio::test]
async fn attachment_upload_with_comment_private_integration() {
    use wiremock::matchers::body_string_contains;
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
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

    let action = bzr::cli::AttachmentAction::Upload(bzr::cli::attachment::UploadArgs {
        bug_id: 42,
        file: upload_file.to_string_lossy().into_owned(),
        summary: Some("test".into()),
        content_type: Some("text/plain".into()),
        private: false,
        no_private: false,
        patch: false,
        no_patch: false,
        comment: Some("sensitive".into()),
        comment_private: true,
        flag: vec![],
    });
    let result = bzr::commands::attachment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
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

    let action = bzr::cli::AttachmentAction::List { bug_id: 42 };
    let mut __io20 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::attachment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io20.writers(),
    )
    .await;
    let output = __io20.out_str().to_string();
    assert!(result.is_ok(), "list should succeed: {result:?}");
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed[0]["is_patch"], true);
}

// ── Attachment update ────────────────────────────────────────────────

#[tokio::test]
async fn attachment_update_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/bug/attachment/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "attachments": [{"id": 99, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::AttachmentAction::Update(bzr::cli::attachment::UpdateArgs {
        id: 99,
        summary: Some("Updated summary".to_string()),
        file_name: None,
        content_type: None,
        obsolete: false,
        no_obsolete: false,
        patch: false,
        no_patch: false,
        private: false,
        no_private: false,
        flag: vec![],
    });
    let result = bzr::commands::attachment::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
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
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/component/10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 10})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::ComponentAction::Update {
        from_json: None,
        id: Some(10),
        name: Some("Updated".to_string()),
        description: None,
        default_assignee: None,
    };
    let result = bzr::commands::component::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
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
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
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

    let action = bzr::cli::ProductAction::View {
        name: "Firefox".to_string(),
    };
    let result = bzr::commands::product::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "product view should succeed: {result:?}");
}

// ── Product create ───────────────────────────────────────────────────

#[tokio::test]
async fn product_create_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/product"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 5})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::ProductAction::Create {
        from_json: None,
        name: Some("NewProduct".to_string()),
        description: Some("A new product".to_string()),
        version: Some("1.0".to_string()),
        is_open: Some(true),
    };
    let result = bzr::commands::product::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "product create should succeed: {result:?}");
}

// ── Product update ───────────────────────────────────────────────────

#[tokio::test]
async fn product_update_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/product/Firefox"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "products": [{"id": 1, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::ProductAction::Update {
        from_json: None,
        name: Some("Firefox".to_string()),
        description: Some("Updated description".to_string()),
        default_milestone: None,
        is_open: None,
    };
    let result = bzr::commands::product::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "product update should succeed: {result:?}");
}

// ── User create ──────────────────────────────────────────────────────

#[tokio::test]
async fn user_create_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 42})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::UserAction::Create {
        from_json: None,
        email: Some("new@example.com".to_string()),
        login: None,
        full_name: Some("New User".to_string()),
        password: None,
    };
    let result = bzr::commands::user::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "user create should succeed: {result:?}");
}

// ── User update ──────────────────────────────────────────────────────

#[tokio::test]
async fn user_update_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/user/alice%40example%2Ecom"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [{"id": 1, "changes": {}}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::UserAction::Update {
        from_json: None,
        user: Some("alice@example.com".to_string()),
        real_name: Some("Alice Updated".to_string()),
        email: None,
        disable_login: None,
        login_denied_text: None,
    };
    let result = bzr::commands::user::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "user update should succeed: {result:?}");
}

// ── Group create ─────────────────────────────────────────────────────

#[tokio::test]
async fn group_create_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("POST"))
        .and(path("/rest/group"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 10})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::GroupAction::Create {
        from_json: None,
        name: Some("testers".to_string()),
        description: Some("Tester group".to_string()),
        is_active: Some(true),
    };
    let result = bzr::commands::group::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "group create should succeed: {result:?}");
}

// ── Group update ─────────────────────────────────────────────────────

#[tokio::test]
async fn group_update_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("PUT"))
        .and(path("/rest/group/testers"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 10, "changes": {}
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = bzr::cli::GroupAction::Update {
        from_json: None,
        group: Some("testers".to_string()),
        description: Some("Updated testers".to_string()),
        is_active: None,
    };
    let result = bzr::commands::group::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "group update should succeed: {result:?}");
}

// ── Group add user ───────────────────────────────────────────────────

#[tokio::test]
async fn group_add_user_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

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
    let result = bzr::commands::group::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "group add-user should succeed: {result:?}");
}

// ── Group remove user ────────────────────────────────────────────────

#[tokio::test]
async fn group_remove_user_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let (_lock, mock, _tmp) = setup_test_env().await;

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
    let result = bzr::commands::group::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "group remove-user should succeed: {result:?}"
    );
}

// ── Group list users ─────────────────────────────────────────────────

#[tokio::test]
async fn group_list_users_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
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

    let action = bzr::cli::GroupAction::ListUsers {
        group: "admin".to_string(),
        details: false,
    };
    let result = bzr::commands::group::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "group list-users should succeed: {result:?}"
    );
}

// ── Config set-server and set-default ────────────────────────────────

#[tokio::test]
async fn config_set_server_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let _lock = ENV_LOCK.lock().await;
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
        api_key: Some("staging-key-abc".to_string()),
        api_key_env: None,
        email: None,
        auth_method: None,
        tls_insecure: false,
        tls_ca_cert: None,
        tls_pin_sha256: None,
        tls_pin_now: false,
        tls_pin_clear: false,
    };
    let result = bzr::commands::config::execute(
        &action,
        None,
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "config set-server should succeed: {result:?}"
    );
}

#[tokio::test]
async fn config_set_default_integration() {
    let mut __cap_io = bzr::test_helpers::CapturedIo::new();
    let _lock = ENV_LOCK.lock().await;
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
    let result = bzr::commands::config::execute(
        &action,
        None,
        bzr::types::OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "config set-default should succeed: {result:?}"
    );
}

// ── CLI-to-execute end-to-end tests ──────────────────────────────────
// These test the full path: CLI arg parsing → command dispatch → API call

/// Parse CLI args and dispatch to the correct command `execute(, &mut __cap_io.writers())` function,
/// exercising the same path as `main.rs::run()`.
async fn dispatch_cli(args: &[&str]) -> bzr::error::Result<()> {
    let cli = bzr::cli::Cli::try_parse_from(args)
        .map_err(|e| bzr::error::BzrError::InputValidation(e.to_string()))?;

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
                Err(bzr::error::BzrError::InputValidation(e.to_string())),
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
                Err(bzr::error::BzrError::InputValidation(e.to_string())),
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
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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
        .and(path("/rest/field/bug/bug_status"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "fields": [{
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
    //
    // Note: FIELD_MAPPINGS iterates whiteboard (idx 7) before
    // resolution (idx 12), so whiteboard gets f1 and resolution
    // gets f2.
    let (_lock, mock, _tmp) = setup_test_env().await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("product", "P"))
        .and(query_param("f1", "status_whiteboard"))
        .and(query_param("o1", "notsubstring"))
        .and(query_param("v1", "wip"))
        .and(query_param("f2", "resolution"))
        .and(query_param("o2", "notequals"))
        .and(query_param("v2", "FIXED"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let mut action = empty_list_action();
    if let bzr::cli::BugAction::List(bzr::cli::ListArgs {
        page_args: _,
        product,
        whiteboard,
        resolution,
        ..
    }) = &mut action
    {
        *product = vec!["P".into()];
        *whiteboard = vec!["!wip".into()];
        *resolution = vec!["!FIXED".into()];
    }
    let mut __io24 = bzr::test_helpers::CapturedIo::new();
    let result = bzr::commands::bug::execute(
        &action,
        Some("test"),
        bzr::types::OutputFormat::Json,
        None,
        &mut __io24.writers(),
    )
    .await;
    let _output = __io24.out_str().to_string();
    assert!(result.is_ok(), "bug list should succeed: {result:?}");
    // wiremock's `expect(1)` enforces that exactly one request matched
    // every query-param matcher above; if any operator or value were
    // wrong, the matcher would miss and the response would be a 404,
    // causing `result` to be `Err`.
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
    let parsed = serde_json::from_str::<serde_json::Value>(out.trim()).unwrap();
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
    let parsed = serde_json::from_str::<serde_json::Value>(out.trim()).unwrap();
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
    let parsed = serde_json::from_str::<serde_json::Value>(out.trim()).unwrap();
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
    let parsed = serde_json::from_str::<serde_json::Value>(out.trim()).unwrap();
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
    let parsed = serde_json::from_str::<serde_json::Value>(out.trim()).unwrap();
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
    let parsed = serde_json::from_str::<serde_json::Value>(out.trim()).unwrap();
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

/// An unknown shell name is rejected at parse time (clap value enum),
/// before any dispatch happens.
#[tokio::test]
async fn completion_rejects_unknown_shell() {
    let parsed = bzr::cli::Cli::try_parse_from(["bzr", "completion", "klingon"]);
    assert!(parsed.is_err(), "clap should reject an unknown shell name");
}

/// #323: the redundant `show` subcommand was removed. Bare `whoami` parses
/// to the unit `Commands::Whoami`, and `whoami show` is now rejected.
#[tokio::test]
async fn whoami_bare_parses_and_show_subcommand_removed() {
    let parsed = bzr::cli::Cli::try_parse_from(["bzr", "whoami"]).expect("bare whoami parses");
    assert!(matches!(parsed.command, bzr::cli::Commands::Whoami));

    let show = bzr::cli::Cli::try_parse_from(["bzr", "whoami", "show"]);
    assert!(show.is_err(), "`whoami show` should no longer be accepted");
}
