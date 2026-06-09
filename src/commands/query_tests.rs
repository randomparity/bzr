#![expect(clippy::unwrap_used)]

use crate::cli::QueryAction;
use crate::config::Config;
use crate::test_helpers::setup_test_env;
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
    }
}

fn run_action(name: &str) -> QueryAction {
    QueryAction::Run {
        name: name.into(),
        limit: None,
        fields: None,
        exclude_fields: None,
        server: None,
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
    }
}

#[tokio::test]
async fn query_save_and_show() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = save_action("test-q");
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a1.writers(),
    )
    .await;
    let _output = __io_a1.out_str().to_string();
    assert!(result.is_ok(), "query save failed: {result:?}");

    let action = QueryAction::Show {
        name: "test-q".into(),
    };
    let mut __io_a2 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a2.writers(),
    )
    .await;
    let output = __io_a2.out_str().to_string();
    assert!(result.is_ok(), "query show failed: {result:?}");
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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
    };
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut __io.writers()).await;
    let _ = __io.out_str().to_string();
    result.unwrap();

    let action = QueryAction::Show {
        name: "comprehensive".into(),
    };
    let mut __io_a3 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a3.writers(),
    )
    .await;
    let output = __io_a3.out_str().to_string();
    result.unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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

    let mut __io2 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &save_action("listed-query"),
        None,
        OutputFormat::Json,
        None,
        &mut __io2.writers(),
    )
    .await;

    let _ = __io2.out_str().to_string();
    result.unwrap();

    let mut __io3 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &QueryAction::List,
        None,
        OutputFormat::Json,
        None,
        &mut __io3.writers(),
    )
    .await;

    let output = __io3.out_str().to_string();
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
    let mut __io_a4 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a4.writers(),
    )
    .await;
    let _output = __io_a4.out_str().to_string();
    assert!(result.is_ok(), "query save failed: {result:?}");

    let action = QueryAction::Show {
        name: "crashes".into(),
    };
    let mut __io_a5 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a5.writers(),
    )
    .await;
    let output = __io_a5.out_str().to_string();
    assert!(result.is_ok(), "query show failed: {result:?}");
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["kind"], "search");
    assert_eq!(parsed["quicksearch"], "crash in tab");
}

#[tokio::test]
async fn query_save_requires_filter() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = empty_save_action("empty", None);
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    assert!(result.is_err(), "saving empty query should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("at least one filter"),
        "expected validation error, got: {err}"
    );
}

#[tokio::test]
async fn query_delete_unknown_errors() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = QueryAction::Delete {
        name: "nonexistent".into(),
    };
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
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
    let mut __io_a6 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a6.writers(),
    )
    .await;
    let _output = __io_a6.out_str().to_string();
    assert!(result.is_ok(), "query list failed: {result:?}");
}

#[tokio::test]
async fn query_run_executes_saved_query() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // First, save a query
    let save_action = product_save_action("run-test", "TestProduct", 10);
    let mut __io_a7 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &save_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a7.writers(),
    )
    .await;
    let _ = __io_a7.out_str().to_string();
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
    let mut __io_a8 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &run_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a8.writers(),
    )
    .await;
    let output = __io_a8.out_str().to_string();
    assert!(result.is_ok(), "query run failed: {result:?}");
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed[0]["id"], 1);
    assert_eq!(parsed[0]["product"], "TestProduct");
}

#[tokio::test]
async fn query_run_honors_saved_custom_fields() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    let mut save_action = product_save_action("custom-fields-test", "TestProduct", 10);
    let QueryAction::Save { fields, .. } = &mut save_action else {
        unreachable!()
    };
    *fields = Some("id,cf_release".into());

    let mut save_io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &save_action,
        None,
        OutputFormat::Json,
        None,
        &mut save_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "query save failed: {result:?}");

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("include_fields", "id,cf_release"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1, "summary": "Test bug", "cf_release": "9.6"}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let run_action = run_action("custom-fields-test");
    let mut run_io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &run_action,
        None,
        OutputFormat::Json,
        None,
        &mut run_io.writers(),
    )
    .await;

    assert!(result.is_ok(), "query run failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(run_io.out_str().trim()).unwrap();
    assert_eq!(parsed[0]["id"], 1);
    assert_eq!(parsed[0]["cf_release"], "9.6");
    assert!(parsed[0].get("summary").is_none());
}

#[tokio::test]
async fn query_run_with_limit_override() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    let save_action = product_save_action("override-test", "TestProduct", 100);
    let mut __io_a9 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &save_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a9.writers(),
    )
    .await;
    let _ = __io_a9.out_str().to_string();
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
    };
    let mut __io_a10 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &run_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a10.writers(),
    )
    .await;
    let _ = __io_a10.out_str().to_string();
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
    };
    let mut __io_a11 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &save_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a11.writers(),
    )
    .await;
    let _ = __io_a11.out_str().to_string();
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
    };
    let mut __io4 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &update_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io4.writers(),
    )
    .await;
    let output = __io4.out_str().to_string();
    assert!(result.is_ok());

    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
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
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let save_action = product_save_action("delete-me", "Firefox", 1);
    let mut __io_a12 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &save_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a12.writers(),
    )
    .await;
    let _ = __io_a12.out_str().to_string();
    assert!(result.is_ok());

    let delete_action = QueryAction::Delete {
        name: "delete-me".into(),
    };
    let mut __io5 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &delete_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io5.writers(),
    )
    .await;
    let output = __io5.out_str().to_string();
    assert!(result.is_ok());
    let parsed = serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed["action"], "deleted");

    let show_action = QueryAction::Show {
        name: "delete-me".into(),
    };
    let err = super::execute(
        &show_action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
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
    };
    let mut __io_a13 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &save_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a13.writers(),
    )
    .await;
    let _ = __io_a13.out_str().to_string();
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
    };
    let mut __io_a14 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &run_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a14.writers(),
    )
    .await;
    let _ = __io_a14.out_str().to_string();
    assert!(result.is_ok());
}

#[tokio::test]
async fn query_run_unknown_errors() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = run_action("nonexistent");
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
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
        let mut __io6 = crate::test_helpers::CapturedIo::new();
        let result = super::execute(
            &save_action(name),
            None,
            OutputFormat::Json,
            None,
            &mut __io6.writers(),
        )
        .await;
        let _ = __io6.out_str().to_string();
        assert!(result.is_ok());
    }

    let mut __io7 = crate::test_helpers::CapturedIo::new();

    let result = super::execute(
        &QueryAction::List,
        None,
        OutputFormat::Table,
        None,
        &mut __io7.writers(),
    )
    .await;

    let _ = __io7.out_str().to_string();
    assert!(result.is_ok());

    let config = Config::load().unwrap();
    let mut names: Vec<&str> = config.queries.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["aaa", "zzz"]);
}

#[tokio::test]
async fn query_show_unknown_errors() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let err = super::execute(
        &QueryAction::Show {
            name: "missing".into(),
        },
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
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
    let mut __io_a15 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &save_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a15.writers(),
    )
    .await;
    let _ = __io_a15.out_str().to_string();
    assert!(result.is_ok());

    // Patch the saved query to have a different server
    Config::update_locked(|config| {
        let query = config.queries.get_mut("server-test").unwrap();
        query.server = Some("other-server".into());
        Ok(())
    })
    .unwrap();

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
    };
    let mut __io_a16 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &run_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a16.writers(),
    )
    .await;
    let _ = __io_a16.out_str().to_string();
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
    let mut __io_a17 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a17.writers(),
    )
    .await;
    let _output = __io_a17.out_str().to_string();
    assert!(result.is_ok(), "query save --from-url failed: {result:?}");

    let config = Config::load().unwrap();
    let saved = &config.queries["url-query"];
    assert_eq!(saved.kind, crate::types::QueryKind::Url);
    assert_eq!(saved.product, vec!["TestProduct"]);
    assert!(!saved.raw_params.is_empty());
    assert!(saved.source_url.is_some());
}

#[tokio::test]
async fn query_save_rejects_malformed_created_since() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = QueryAction::Save {
        name: "bad".into(),
        from_url: None,
        search: None,
        product: vec!["Firefox".into()],
        component: vec![],
        status: vec![],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        limit: None,
        fields: None,
        exclude_fields: None,
        created_since: Some("garbage".into()),
        changed_since: None,
        whiteboard: vec![],
        target_milestone: vec![],
        version: vec![],
        op_sys: vec![],
        platform: vec![],
        resolution: vec![],
        qa_contact: vec![],
        url: vec![],
    };

    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(err.to_string().contains("--created-since"));
}

#[tokio::test]
async fn query_save_stores_canonical_date_forms() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = QueryAction::Save {
        name: "recent".into(),
        from_url: None,
        search: None,
        product: vec!["Firefox".into()],
        component: vec![],
        status: vec![],
        assignee: vec![],
        creator: vec![],
        priority: vec![],
        severity: vec![],
        limit: None,
        fields: None,
        exclude_fields: None,
        created_since: Some("2026-04-01".into()),
        changed_since: Some("2026-04-15T12:00:00Z".into()),
        whiteboard: vec![],
        target_milestone: vec![],
        version: vec![],
        op_sys: vec![],
        platform: vec![],
        resolution: vec![],
        qa_contact: vec![],
        url: vec![],
    };
    let mut __io8 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io8.writers(),
    )
    .await;
    let _ = __io8.out_str().to_string();
    result.unwrap();

    let cfg = Config::load().unwrap();
    let q = cfg.queries.get("recent").unwrap();
    assert_eq!(q.creation_time.as_deref(), Some("2026-04-01T00:00:00Z"));
    assert_eq!(q.last_change_time.as_deref(), Some("2026-04-15T12:00:00Z"));
}

#[tokio::test]
async fn query_save_accepts_date_only_query() {
    // SavedQuery::has_filters() must recognize date-only queries so the
    // "query must have at least one filter set" rejection does not fire.
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = QueryAction::Save {
        name: "date-only".into(),
        from_url: None,
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
        created_since: Some("2026-04-01".into()),
        changed_since: None,
        whiteboard: vec![],
        target_milestone: vec![],
        version: vec![],
        op_sys: vec![],
        platform: vec![],
        resolution: vec![],
        qa_contact: vec![],
        url: vec![],
    };
    let mut __io9 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io9.writers(),
    )
    .await;
    let _ = __io9.out_str().to_string();
    result.unwrap();
    let cfg = Config::load().unwrap();
    assert!(cfg.queries.contains_key("date-only"));
}

#[tokio::test]
async fn query_run_rejects_malformed_created_since_override() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    // Pre-seed a saved query so the not-found branch doesn't fire first.
    Config::update_locked(move |c| {
        c.queries.insert(
            "recent".into(),
            crate::types::SavedQuery {
                product: vec!["Firefox".into()],
                ..crate::types::SavedQuery::default()
            },
        );
        Ok(())
    })
    .unwrap();

    let action = QueryAction::Run {
        name: "recent".into(),
        limit: None,
        fields: None,
        exclude_fields: None,
        server: None,
        created_since: Some("not-a-date".into()),
        changed_since: None,
        whiteboard: vec![],
        target_milestone: vec![],
        version: vec![],
        op_sys: vec![],
        platform: vec![],
        resolution: vec![],
        qa_contact: vec![],
        url: vec![],
    };
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __cap_io.writers(),
    )
    .await;
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(err.to_string().contains("--created-since"));
}

#[tokio::test]
async fn query_save_persists_158_field_filters() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = QueryAction::Save {
        name: "field-filters".into(),
        from_url: None,
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
        created_since: None,
        changed_since: None,
        whiteboard: vec!["needs-review".into()],
        target_milestone: vec!["5.0".into()],
        version: vec!["9.4".into()],
        op_sys: vec!["Linux".into()],
        platform: vec!["x86_64".into()],
        resolution: vec!["FIXED".into()],
        qa_contact: vec!["qa@example.com".into()],
        url: vec!["github.com/foo".into()],
    };
    let mut __io_a18 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a18.writers(),
    )
    .await;
    let _output = __io_a18.out_str().to_string();
    assert!(result.is_ok(), "save failed: {result:?}");

    let cfg = Config::load().unwrap();
    let q = cfg.queries.get("field-filters").unwrap();
    assert_eq!(q.whiteboard, vec!["needs-review"]);
    assert_eq!(q.target_milestone, vec!["5.0"]);
    assert_eq!(q.version, vec!["9.4"]);
    assert_eq!(q.op_sys, vec!["Linux"]);
    assert_eq!(q.platform, vec!["x86_64"]);
    assert_eq!(q.resolution, vec!["FIXED"]);
    assert_eq!(q.qa_contact, vec!["qa@example.com"]);
    assert_eq!(q.url, vec!["github.com/foo"]);
}

#[tokio::test]
async fn query_save_accepts_whiteboard_only_filter() {
    // Regression: SavedQuery::has_filters() must accept a query whose
    // ONLY filter is one of the 8 new fields. Otherwise saving such
    // a query would fail with "query must have at least one filter
    // set".
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = QueryAction::Save {
        name: "wb-only".into(),
        from_url: None,
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
        created_since: None,
        changed_since: None,
        whiteboard: vec!["wip".into()],
        target_milestone: vec![],
        version: vec![],
        op_sys: vec![],
        platform: vec![],
        resolution: vec![],
        qa_contact: vec![],
        url: vec![],
    };
    let mut __io_a19 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a19.writers(),
    )
    .await;
    let _output = __io_a19.out_str().to_string();
    assert!(
        result.is_ok(),
        "save with whiteboard-only filter must succeed: {result:?}"
    );
}

#[tokio::test]
async fn query_run_overrides_replace_saved_field_filters() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // Save a query with whiteboard=original and resolution=FIXED.
    let save_action = QueryAction::Save {
        name: "field-override-test".into(),
        from_url: None,
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
        created_since: None,
        changed_since: None,
        whiteboard: vec!["original".into()],
        target_milestone: vec![],
        version: vec![],
        op_sys: vec![],
        platform: vec![],
        resolution: vec!["FIXED".into()],
        qa_contact: vec![],
        url: vec![],
    };
    let mut __io_a20 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &save_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a20.writers(),
    )
    .await;
    let _ = __io_a20.out_str().to_string();
    assert!(result.is_ok(), "save failed: {result:?}");

    // The run must hit the wire with the override values, not the saved ones.
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("whiteboard", "overridden"))
        .and(query_param("resolution", "WONTFIX"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let run_action = QueryAction::Run {
        name: "field-override-test".into(),
        limit: None,
        fields: None,
        exclude_fields: None,
        server: None,
        created_since: None,
        changed_since: None,
        whiteboard: vec!["overridden".into()],
        target_milestone: vec![],
        version: vec![],
        op_sys: vec![],
        platform: vec![],
        resolution: vec!["WONTFIX".into()],
        qa_contact: vec![],
        url: vec![],
    };
    let mut __io_a21 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &run_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a21.writers(),
    )
    .await;
    let _ = __io_a21.out_str().to_string();
    assert!(result.is_ok(), "run failed: {result:?}");
}

#[tokio::test]
async fn query_run_empty_override_keeps_saved_field_filter() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    let save_action = QueryAction::Save {
        name: "saved-wb".into(),
        from_url: None,
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
        created_since: None,
        changed_since: None,
        whiteboard: vec!["original".into()],
        target_milestone: vec![],
        version: vec![],
        op_sys: vec![],
        platform: vec![],
        resolution: vec![],
        qa_contact: vec![],
        url: vec![],
    };
    let mut __io_a22 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &save_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a22.writers(),
    )
    .await;
    let _ = __io_a22.out_str().to_string();
    assert!(result.is_ok(), "save failed: {result:?}");

    // No --whiteboard override on run: the saved value must reach the wire.
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("whiteboard", "original"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let run_action = run_action("saved-wb");
    let mut __io_a23 = crate::test_helpers::CapturedIo::new();
    let result = super::execute(
        &run_action,
        None,
        OutputFormat::Json,
        None,
        &mut __io_a23.writers(),
    )
    .await;
    let _ = __io_a23.out_str().to_string();
    assert!(result.is_ok(), "run failed: {result:?}");
}
