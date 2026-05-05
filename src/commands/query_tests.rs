#![expect(clippy::unwrap_used)]

use crate::cli::QueryAction;
use crate::config::Config;
use crate::test_helpers::{capture_stdout, setup_test_env};
use crate::types::OutputFormat;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn save_action(name: &str) -> QueryAction {
    QueryAction::Save {
        name: name.into(),
        from_url: None,
        search: None,
        product: vec!["Firefox".into()],
        component: vec![],
        status: vec!["NEW".into()],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        limit: Some(25),
        fields: None,
        exclude_fields: None,
    }
}

/// Build a Save action for a single-product query with no status filters.
fn product_save_action(name: &str, product: &str, limit: u32) -> QueryAction {
    QueryAction::Save {
        name: name.into(),
        from_url: None,
        search: None,
        product: vec![product.into()],
        component: vec![],
        status: vec![],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        limit: Some(limit),
        fields: None,
        exclude_fields: None,
    }
}

fn empty_save_action(name: &str, search: Option<String>) -> QueryAction {
    QueryAction::Save {
        name: name.into(),
        from_url: None,
        search,
        product: vec![],
        component: vec![],
        status: vec![],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        limit: None,
        fields: None,
        exclude_fields: None,
    }
}

fn url_save_action(name: &str, url: String) -> QueryAction {
    QueryAction::Save {
        name: name.into(),
        from_url: Some(url),
        search: None,
        product: vec![],
        component: vec![],
        status: vec![],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        limit: None,
        fields: None,
        exclude_fields: None,
    }
}

fn run_action(name: &str) -> QueryAction {
    QueryAction::Run {
        name: name.into(),
        limit: None,
        fields: None,
        exclude_fields: None,
        server: None,
    }
}

#[tokio::test]
async fn query_save_and_show() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = save_action("test-q");
    let (result, _output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "query save failed: {result:?}");

    let action = QueryAction::Show {
        name: "test-q".into(),
    };
    let (result, output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "query show failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["name"], "test-q");
    assert_eq!(parsed["kind"], "list");
    assert_eq!(parsed["product"][0], "Firefox");
}

#[tokio::test]
async fn query_save_persists_every_field() {
    // Every Save-action field must round-trip into the persisted
    // SavedQuery and be visible in `query show`.
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = QueryAction::Save {
        name: "comprehensive".into(),
        from_url: None,
        search: None,
        product: vec!["Firefox".into()],
        component: vec!["General".into()],
        status: vec!["NEW".into()],
        assignee: vec!["dev@test.com".into()],
        creator: vec!["reporter@test.com".into()],
        priority: vec!["P1".into()],
        severity: vec!["major".into()],
        limit: Some(7),
        fields: Some("id,summary".into()),
        exclude_fields: Some("comments".into()),
    };
    let (result, _) = capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    result.unwrap();

    let action = QueryAction::Show {
        name: "comprehensive".into(),
    };
    let (result, output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    result.unwrap();
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["product"][0], "Firefox");
    assert_eq!(parsed["component"][0], "General");
    assert_eq!(parsed["status"][0], "NEW");
    assert_eq!(parsed["assignee"][0], "dev@test.com");
    assert_eq!(parsed["creator"][0], "reporter@test.com");
    assert_eq!(parsed["priority"][0], "P1");
    assert_eq!(parsed["severity"][0], "major");
    assert_eq!(parsed["limit"], 7);
    assert_eq!(parsed["fields"], "id,summary");
    assert_eq!(parsed["exclude_fields"], "comments");
}

#[tokio::test]
async fn query_list_emits_saved_query_names() {
    // Saved queries must appear in `query list` output.
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let (result, _) = capture_stdout(super::execute(
        &save_action("listed-query"),
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    result.unwrap();

    let (result, output) = capture_stdout(super::execute(
        &QueryAction::List,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    result.unwrap();
    assert!(
        output.contains("listed-query"),
        "expected query name in list output; got: {output:?}"
    );
}

#[tokio::test]
async fn query_save_search_kind() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = empty_save_action("crashes", Some("crash in tab".into()));
    let (result, _output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "query save failed: {result:?}");

    let action = QueryAction::Show {
        name: "crashes".into(),
    };
    let (result, output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "query show failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["kind"], "search");
    assert_eq!(parsed["quicksearch"], "crash in tab");
}

#[tokio::test]
async fn query_save_requires_filter() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = empty_save_action("empty", None);
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_err(), "saving empty query should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("at least one filter"),
        "expected validation error, got: {err}"
    );
}

#[tokio::test]
async fn query_delete_unknown_errors() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = QueryAction::Delete {
        name: "nonexistent".into(),
    };
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_err(), "deleting unknown query should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not found"),
        "expected not-found error, got: {err}"
    );
}

#[tokio::test]
async fn query_list_empty() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = QueryAction::List;
    let (result, _output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "query list failed: {result:?}");
}

#[tokio::test]
async fn query_run_executes_saved_query() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // First, save a query
    let save_action = product_save_action("run-test", "TestProduct", 10);
    let (result, _) =
        capture_stdout(super::execute(&save_action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "query save failed: {result:?}");

    // Mock the bug search endpoint
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 1,
                "summary": "Test bug",
                "status": "NEW",
                "product": "TestProduct",
                "component": "General"
            }]
        })))
        .mount(&mock)
        .await;

    // Run the saved query
    let run_action = run_action("run-test");
    let (result, output) =
        capture_stdout(super::execute(&run_action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "query run failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed[0]["id"], 1);
    assert_eq!(parsed[0]["product"], "TestProduct");
}

#[tokio::test]
async fn query_run_with_limit_override() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    let save_action = product_save_action("override-test", "TestProduct", 100);
    let (result, _) =
        capture_stdout(super::execute(&save_action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok());

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("limit", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let run_action = QueryAction::Run {
        name: "override-test".into(),
        limit: Some(5),
        fields: None,
        exclude_fields: None,
        server: None,
    };
    let (result, _) =
        capture_stdout(super::execute(&run_action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "query run with override failed: {result:?}");
}

#[tokio::test]
async fn query_save_existing_entry_reports_updated() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let save_action = QueryAction::Save {
        name: "existing".into(),
        from_url: None,
        search: None,
        product: vec!["Firefox".into()],
        component: vec![],
        status: vec!["NEW".into()],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        limit: Some(10),
        fields: None,
        exclude_fields: None,
    };
    let (result, _) =
        capture_stdout(super::execute(&save_action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok());

    let update_action = QueryAction::Save {
        name: "existing".into(),
        from_url: None,
        search: Some("updated".into()),
        product: vec![],
        component: vec![],
        status: vec![],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        limit: Some(5),
        fields: None,
        exclude_fields: None,
    };
    let (result, output) = capture_stdout(super::execute(
        &update_action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok());

    let parsed = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["name"], "existing");
    assert_eq!(parsed["action"], "updated");

    let config = Config::load().unwrap();
    let saved = &config.queries["existing"];
    assert_eq!(saved.quicksearch.as_deref(), Some("updated"));
    assert_eq!(saved.limit, Some(5));
    assert!(saved.product.is_empty());
}

#[tokio::test]
async fn query_delete_removes_saved_query() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let save_action = product_save_action("delete-me", "Firefox", 1);
    let (result, _) =
        capture_stdout(super::execute(&save_action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok());

    let delete_action = QueryAction::Delete {
        name: "delete-me".into(),
    };
    let (result, output) = capture_stdout(super::execute(
        &delete_action,
        None,
        OutputFormat::Json,
        None,
    ))
    .await;
    assert!(result.is_ok());
    let parsed = crate::test_helpers::extract_json(&output);
    assert_eq!(parsed["action"], "deleted");

    let show_action = QueryAction::Show {
        name: "delete-me".into(),
    };
    let err = super::execute(&show_action, None, OutputFormat::Json, None)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
async fn query_run_applies_field_overrides() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    let save_action = QueryAction::Save {
        name: "fields-test".into(),
        from_url: None,
        search: None,
        product: vec!["TestProduct".into()],
        component: vec![],
        status: vec![],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        limit: Some(10),
        fields: Some("id,status".into()),
        exclude_fields: Some("cc".into()),
    };
    let (result, _) =
        capture_stdout(super::execute(&save_action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok());

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("include_fields", "id,summary"))
        .and(query_param("exclude_fields", "comments"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let run_action = QueryAction::Run {
        name: "fields-test".into(),
        limit: None,
        fields: Some("id,summary".into()),
        exclude_fields: Some("comments".into()),
        server: None,
    };
    let (result, _) =
        capture_stdout(super::execute(&run_action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn query_run_unknown_errors() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = run_action("nonexistent");
    let result = super::execute(&action, None, OutputFormat::Json, None).await;
    assert!(result.is_err(), "running unknown query should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not found"),
        "expected not-found error, got: {err}"
    );
}

#[tokio::test]
async fn query_list_table_sorts_entries_by_name() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    for name in ["zzz", "aaa"] {
        let (result, _) = capture_stdout(super::execute(
            &save_action(name),
            None,
            OutputFormat::Json,
            None,
        ))
        .await;
        assert!(result.is_ok());
    }

    let (result, _) = capture_stdout(super::execute(
        &QueryAction::List,
        None,
        OutputFormat::Table,
        None,
    ))
    .await;
    assert!(result.is_ok());

    let config = Config::load().unwrap();
    let mut names: Vec<&str> = config.queries.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["aaa", "zzz"]);
}

#[tokio::test]
async fn query_show_unknown_errors() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let err = super::execute(
        &QueryAction::Show {
            name: "missing".into(),
        },
        None,
        OutputFormat::Json,
        None,
    )
    .await
    .unwrap_err();

    assert!(err.to_string().contains("query 'missing' not found"));
}

#[tokio::test]
async fn query_run_with_server_override() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // Save a query that records a different server than the mock
    let save_action = save_action("server-test");
    let (result, _) =
        capture_stdout(super::execute(&save_action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok());

    // Patch the saved query to have a different server
    let mut config = Config::load().unwrap();
    let query = config.queries.get_mut("server-test").unwrap();
    query.server = Some("other-server".into());
    config.save().unwrap();

    // Mount a mock that expects exactly 1 request
    let mock_guard = Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount_as_scoped(&mock)
        .await;

    // Run with --server override pointing to the mock server ("test")
    let run_action = QueryAction::Run {
        name: "server-test".into(),
        limit: None,
        fields: None,
        exclude_fields: None,
        server: Some("test".into()),
    };
    let (result, _) =
        capture_stdout(super::execute(&run_action, None, OutputFormat::Json, None)).await;
    assert!(
        result.is_ok(),
        "query run with server override failed: {result:?}"
    );

    // Drop the scoped mock to trigger the expect(1) assertion
    drop(mock_guard);
}

#[tokio::test]
async fn query_save_from_url() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    let server_url = mock.uri();
    let url = format!(
        "{server_url}/buglist.cgi?product=TestProduct&f1=qa_contact&o1=changedfrom&v1=user%40example.com"
    );
    let action = url_save_action("url-query", url);
    let (result, _output) =
        capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
    assert!(result.is_ok(), "query save --from-url failed: {result:?}");

    let config = Config::load().unwrap();
    let saved = &config.queries["url-query"];
    assert_eq!(saved.kind, crate::types::QueryKind::Url);
    assert_eq!(saved.product, vec!["TestProduct"]);
    assert!(!saved.raw_params.is_empty());
    assert!(saved.source_url.is_some());
}
