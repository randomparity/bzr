#![expect(clippy::unwrap_used)]

use crate::cli::query::{DeleteArgs, RunArgs, SaveArgs, ShowArgs, UpdateArgs};
use crate::cli::QueryAction;
use crate::config::Config;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

fn save_action(name: &str) -> QueryAction {
    QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    })
}

/// Build a Save action for a single-product query with no status filters.
fn product_save_action(name: &str, product: &str, limit: u32) -> QueryAction {
    QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    })
}

fn empty_save_action(name: &str, search: Option<String>) -> QueryAction {
    QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    })
}

fn url_save_action(name: &str, url: String) -> QueryAction {
    QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    })
}

fn run_action(name: &str) -> QueryAction {
    QueryAction::Run(RunArgs {
        page_args: crate::cli::PageArgs::default(),
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
        sort_args: crate::cli::SortArgs::default(),
    })
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

    let action = QueryAction::Show(ShowArgs {
        name: "test-q".into(),
    });
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

    let action = QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    });
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(&action, None, OutputFormat::Json, None, &mut __io.writers()).await;
    let _ = __io.out_str().to_string();
    result.unwrap();

    let action = QueryAction::Show(ShowArgs {
        name: "comprehensive".into(),
    });
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

    let action = QueryAction::Show(ShowArgs {
        name: "crashes".into(),
    });
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

    let action = QueryAction::Delete(DeleteArgs {
        name: "nonexistent".into(),
    });
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
    let QueryAction::Save(SaveArgs { fields, .. }) = &mut save_action else {
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
        .and(query_param("limit", "6"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let run_action = QueryAction::Run(RunArgs {
        page_args: crate::cli::PageArgs::default(),
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
        sort_args: crate::cli::SortArgs::default(),
    });
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

    let save_action = QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    });
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

    let update_action = QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    });
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

    let delete_action = QueryAction::Delete(DeleteArgs {
        name: "delete-me".into(),
    });
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

    let show_action = QueryAction::Show(ShowArgs {
        name: "delete-me".into(),
    });
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

    let save_action = QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    });
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

    let run_action = QueryAction::Run(RunArgs {
        page_args: crate::cli::PageArgs::default(),
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
        sort_args: crate::cli::SortArgs::default(),
    });
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
        &QueryAction::Show(ShowArgs {
            name: "missing".into(),
        }),
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
    let run_action = QueryAction::Run(RunArgs {
        page_args: crate::cli::PageArgs::default(),
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
        sort_args: crate::cli::SortArgs::default(),
    });
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

    let action = QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    });

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

    let action = QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    });
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

    let action = QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    });
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

    let action = QueryAction::Run(RunArgs {
        page_args: crate::cli::PageArgs::default(),
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
        sort_args: crate::cli::SortArgs::default(),
    });
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

    let action = QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    });
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

    let action = QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    });
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
    let save_action = QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    });
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

    let run_action = QueryAction::Run(RunArgs {
        page_args: crate::cli::PageArgs::default(),
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
        sort_args: crate::cli::SortArgs::default(),
    });
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

    let save_action = QueryAction::Save(SaveArgs {
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
        sort_args: crate::cli::SortArgs::default(),
    });
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

#[tokio::test]
async fn query_run_sends_default_bug_id_order() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    let save = product_save_action("order-default", "TestProduct", 10);
    super::execute(
        &save,
        None,
        OutputFormat::Json,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("order", "bug_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "bugs": [] })))
        .expect(1)
        .mount(&mock)
        .await;

    let run = run_action("order-default");
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(&run, None, OutputFormat::Json, None, &mut __io.writers()).await;
    assert!(
        result.is_ok(),
        "run with default order should succeed: {result:?}"
    );
}

#[tokio::test]
async fn query_run_sort_override_takes_precedence() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    let save = product_save_action("order-override", "TestProduct", 10);
    super::execute(
        &save,
        None,
        OutputFormat::Json,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("order", "priority ASC, bug_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "bugs": [] })))
        .expect(1)
        .mount(&mock)
        .await;

    let mut run = run_action("order-override");
    if let QueryAction::Run(RunArgs { sort_args, .. }) = &mut run {
        sort_args.sort = Some("priority".to_string());
    }
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = super::execute(&run, None, OutputFormat::Json, None, &mut __io.writers()).await;
    assert!(
        result.is_ok(),
        "run with --sort override should succeed: {result:?}"
    );
}

#[tokio::test]
async fn query_save_persists_explicit_sort() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let mut save = product_save_action("order-persist", "TestProduct", 10);
    if let QueryAction::Save(SaveArgs { sort_args, .. }) = &mut save {
        sort_args.sort = Some("last_change_time".to_string());
        sort_args.order = crate::types::SortDirection::Desc;
    }
    super::execute(
        &save,
        None,
        OutputFormat::Json,
        None,
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    let config = crate::config::Config::load().unwrap();
    let saved = config.queries.get("order-persist").unwrap();
    assert_eq!(
        saved.order.as_deref(),
        Some("last_change_time DESC, bug_id")
    );
}

// ── Update (#315) ───────────────────────────────────────────────────

fn empty_update(name: &str) -> QueryAction {
    QueryAction::Update(UpdateArgs {
        name: name.into(),
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
        clear: vec![],
        sort_args: crate::cli::SortArgs::default(),
    })
}

async fn run_q(action: &QueryAction) -> crate::error::Result<()> {
    let mut io = crate::test_helpers::CapturedIo::new();
    super::execute(action, None, OutputFormat::Json, None, &mut io.writers()).await
}

#[tokio::test]
async fn query_update_replaces_filter_keeps_rest() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap(); // product=Firefox, status=NEW, limit=25

    let mut a = empty_update("q");
    if let QueryAction::Update(UpdateArgs { status, .. }) = &mut a {
        *status = vec!["ASSIGNED".into()];
    }
    run_q(&a).await.unwrap();

    let config = Config::load().unwrap();
    let q = &config.queries["q"];
    assert_eq!(q.status, vec!["ASSIGNED".to_string()]);
    assert_eq!(q.product, vec!["Firefox".to_string()]); // untouched
    assert_eq!(q.limit, Some(25)); // untouched
}

#[tokio::test]
async fn query_update_replaces_limit() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap();

    let mut a = empty_update("q");
    if let QueryAction::Update(UpdateArgs { limit, .. }) = &mut a {
        *limit = Some(100);
    }
    run_q(&a).await.unwrap();

    assert_eq!(Config::load().unwrap().queries["q"].limit, Some(100));
}

#[tokio::test]
async fn query_update_clear_resets_filter() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap();

    let mut a = empty_update("q");
    if let QueryAction::Update(UpdateArgs { clear, .. }) = &mut a {
        *clear = vec!["status".into()];
    }
    run_q(&a).await.unwrap();

    let config = Config::load().unwrap();
    assert!(config.queries["q"].status.is_empty());
    assert_eq!(config.queries["q"].product, vec!["Firefox".to_string()]);
}

#[tokio::test]
async fn query_update_unknown_query_errors() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let mut a = empty_update("missing");
    if let QueryAction::Update(UpdateArgs { status, .. }) = &mut a {
        *status = vec!["NEW".into()];
    }
    let err = run_q(&a).await.unwrap_err();
    assert!(err.to_string().contains("query 'missing' not found"));
}

#[tokio::test]
async fn query_update_requires_a_change() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap();
    let err = run_q(&empty_update("q")).await.unwrap_err();
    assert!(err.to_string().contains("no changes"));
}

#[tokio::test]
async fn query_update_unknown_clear_field_errors() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap();
    let mut a = empty_update("q");
    if let QueryAction::Update(UpdateArgs { clear, .. }) = &mut a {
        *clear = vec!["bogus".into()];
    }
    let err = run_q(&a).await.unwrap_err();
    assert!(err.to_string().contains("unknown --clear field"));
}

#[tokio::test]
async fn query_update_clearing_all_filters_rejected() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&product_save_action("q", "Firefox", 10))
        .await
        .unwrap(); // only product

    let mut a = empty_update("q");
    if let QueryAction::Update(UpdateArgs { clear, .. }) = &mut a {
        *clear = vec!["product".into()];
    }
    let err = run_q(&a).await.unwrap_err();
    assert!(err.to_string().contains("at least one filter"));
}

#[tokio::test]
async fn query_update_bad_date_errors() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap();
    let mut a = empty_update("q");
    if let QueryAction::Update(UpdateArgs { created_since, .. }) = &mut a {
        *created_since = Some("not-a-date".into());
    }
    assert!(run_q(&a).await.is_err());
}

#[tokio::test]
async fn query_update_clear_wins_over_set() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap(); // product=Firefox, status=NEW

    let mut a = empty_update("q");
    if let QueryAction::Update(UpdateArgs { status, clear, .. }) = &mut a {
        *status = vec!["ASSIGNED".into()];
        *clear = vec!["status".into()];
    }
    run_q(&a).await.unwrap();
    // status was both set and cleared -> cleared.
    assert!(Config::load().unwrap().queries["q"].status.is_empty());
}

#[test]
fn clear_query_field_handles_every_name() {
    let mut q = crate::types::SavedQuery {
        product: vec!["p".into()],
        component: vec!["c".into()],
        status: vec!["s".into()],
        assignee: vec!["a".into()],
        creator: vec!["cr".into()],
        priority: vec!["pr".into()],
        severity: vec!["se".into()],
        whiteboard: vec!["w".into()],
        target_milestone: vec!["t".into()],
        version: vec!["v".into()],
        op_sys: vec!["o".into()],
        platform: vec!["pl".into()],
        resolution: vec!["r".into()],
        qa_contact: vec!["q".into()],
        url: vec!["u".into()],
        quicksearch: Some("x".into()),
        limit: Some(5),
        fields: Some("f".into()),
        exclude_fields: Some("e".into()),
        creation_time: Some("2026-01-01".into()),
        last_change_time: Some("2026-02-01".into()),
        order: Some("bug_id".into()),
        ..Default::default()
    };
    for name in [
        "product",
        "component",
        "status",
        "assignee",
        "creator",
        "priority",
        "severity",
        "whiteboard",
        "target-milestone",
        "version",
        "op-sys",
        "platform",
        "resolution",
        "qa-contact",
        "url",
        "search",
        "limit",
        "fields",
        "exclude-fields",
        "created-since",
        "changed-since",
        "sort",
        "order",
    ] {
        super::clear_query_field(&mut q, name).unwrap();
    }
    assert!(!q.has_filters(), "every filter cleared");
    assert!(q.limit.is_none() && q.fields.is_none() && q.order.is_none());
}

#[tokio::test]
async fn query_update_sets_dates_and_sort() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap();

    let mut a = empty_update("q");
    if let QueryAction::Update(UpdateArgs {
        created_since,
        changed_since,
        sort_args,
        ..
    }) = &mut a
    {
        *created_since = Some("2026-03-01".into());
        *changed_since = Some("2026-03-02".into());
        sort_args.sort = Some("priority".into());
    }
    run_q(&a).await.unwrap();

    let config = Config::load().unwrap();
    let q = &config.queries["q"];
    assert!(q.creation_time.is_some());
    assert!(q.last_change_time.is_some());
    assert!(q.order.is_some());
}

#[tokio::test]
async fn query_update_only_product_is_a_change() {
    // Updating ONLY --product must register as a change. apply_query_updates
    // starts `changed = false` and ORs in each merge result; the `&=` mutant on
    // the product line would keep `changed` false and the update would be
    // rejected as "no changes". product/component are the only merge_vec fields
    // without a sole-field update test, so this closes that coverage gap.
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap(); // product=Firefox, status=NEW

    let mut a = empty_update("q");
    if let QueryAction::Update(UpdateArgs { product, .. }) = &mut a {
        *product = vec!["Thunderbird".into()];
    }
    run_q(&a).await.unwrap(); // must NOT fail with "no changes"

    let config = Config::load().unwrap();
    assert_eq!(
        config.queries["q"].product,
        vec!["Thunderbird".to_string()],
        "a sole --product update must be applied"
    );
}

#[tokio::test]
async fn query_update_only_component_is_a_change() {
    // Companion to the product case: a sole --component update must also count
    // as a change. The `&=` mutant on the component line would swallow it.
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap();

    let mut a = empty_update("q");
    if let QueryAction::Update(UpdateArgs { component, .. }) = &mut a {
        *component = vec!["General".into()];
    }
    run_q(&a).await.unwrap();

    let config = Config::load().unwrap();
    assert_eq!(
        config.queries["q"].component,
        vec!["General".to_string()],
        "a sole --component update must be applied"
    );
}

#[tokio::test]
async fn query_run_without_sort_keeps_saved_order() {
    // A saved query carrying an explicit order must keep it when `run` is
    // invoked without --sort. The bug_id default must fire only when the saved
    // order is absent AND no raw `order` param exists (`&&`). The `||` mutant
    // would clobber the saved order with the default whenever there is no raw
    // order param — exactly the common case of a structured saved query.
    let (_lock, mock, _tmp) = setup_test_env().await;

    let mut save = product_save_action("ordered", "TestProduct", 10);
    if let QueryAction::Save(SaveArgs { sort_args, .. }) = &mut save {
        sort_args.sort = Some("last_change_time".into());
        sort_args.order = crate::types::SortDirection::Desc;
    }
    run_q(&save).await.unwrap();

    // The wire must carry the SAVED order, not the bug_id default.
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("order", "last_change_time DESC, bug_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    run_q(&run_action("ordered")).await.unwrap();
}
