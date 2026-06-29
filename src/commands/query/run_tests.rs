#![expect(clippy::unwrap_used)]

use std::path::PathBuf;

use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, ResponseTemplate};

use crate::cli::{
    BugActorFilterArgs, BugFilterArgs, QueryAction, QueryRunFilterArgs, RunArgs, SaveArgs,
};
use crate::config::Config;
use crate::error::Result;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn current_config_path() -> PathBuf {
    Config::path_at(None).unwrap()
}

fn update_config(mutator: impl FnOnce(&mut Config) -> Result<()>) -> Result<Config> {
    let path = current_config_path();
    Config::update_locked_at(Some(&path), mutator)
}

fn save_action(name: &str) -> QueryAction {
    QueryAction::Save(SaveArgs {
        name: name.into(),
        from_url: None,
        search: None,
        filters: BugFilterArgs {
            product: vec!["Firefox".into()],
            component: vec![],
            status: vec!["NEW".into()],
            priority: vec![],
            severity: vec![],
            whiteboard: vec![],
            target_milestone: vec![],
            version: vec![],
            op_sys: vec![],
            platform: vec![],
            resolution: vec![],
            qa_contact: vec![],
            url: vec![],
        },
        actor_filters: BugActorFilterArgs {
            assignee: vec![],
            creator: vec![],
        },
        limit: Some(25),
        fields: None,
        exclude_fields: None,
        created_since: None,
        changed_since: None,

        sort_args: crate::cli::SortArgs::default(),
    })
}

/// Build a Save action for a single-product query with no status filters.
fn product_save_action(name: &str, product: &str, limit: u32) -> QueryAction {
    QueryAction::Save(SaveArgs {
        name: name.into(),
        from_url: None,
        search: None,
        filters: BugFilterArgs {
            product: vec![product.into()],
            component: vec![],
            status: vec![],
            priority: vec![],
            severity: vec![],
            whiteboard: vec![],
            target_milestone: vec![],
            version: vec![],
            op_sys: vec![],
            platform: vec![],
            resolution: vec![],
            qa_contact: vec![],
            url: vec![],
        },
        actor_filters: BugActorFilterArgs {
            assignee: vec![],
            creator: vec![],
        },
        limit: Some(limit),
        fields: None,
        exclude_fields: None,
        created_since: None,
        changed_since: None,

        sort_args: crate::cli::SortArgs::default(),
    })
}

fn run_action(name: &str) -> QueryAction {
    QueryAction::Run(RunArgs {
        page_args: crate::cli::PageArgs::default(),
        name: name.into(),
        count: false,
        limit: None,
        fields: None,
        exclude_fields: None,
        server: None,
        created_since: None,
        changed_since: None,
        filters: QueryRunFilterArgs {
            whiteboard: vec![],
            target_milestone: vec![],
            version: vec![],
            op_sys: vec![],
            platform: vec![],
            resolution: vec![],
            qa_contact: vec![],
        },
        url: vec![],
        sort_args: crate::cli::SortArgs::default(),
    })
}

fn count_run_action(name: &str) -> QueryAction {
    let mut action = run_action(name);
    let QueryAction::Run(RunArgs { count, .. }) = &mut action else {
        unreachable!()
    };
    *count = true;
    action
}

async fn run_q(action: &QueryAction) -> Result<()> {
    let mut io = crate::test_helpers::CapturedIo::new();
    crate::commands::query::execute(
        action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
}

#[tokio::test]
async fn query_run_executes_saved_query() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // First, save a query
    let save_action = product_save_action("run-test", "TestProduct", 10);
    let mut __io_a7 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &save_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = crate::commands::query::execute(
        &run_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a8.writers(),
    )
    .await;
    let output = __io_a8.out_str().to_string();
    assert!(result.is_ok(), "query run failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
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
    let result = crate::commands::query::execute(
        &save_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = crate::commands::query::execute(
        &run_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut run_io.writers(),
    )
    .await;

    assert!(result.is_ok(), "query run failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(run_io.out_str());
    assert_eq!(parsed[0]["id"], 1);
    assert_eq!(parsed[0]["cf_release"], "9.6");
    assert!(parsed[0].get("summary").is_none());
}

#[tokio::test]
async fn query_run_count_json_emits_count_object() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    let save_action = product_save_action("count-json-test", "TestProduct", 25);
    let mut save_io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &save_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut save_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "query save failed: {result:?}");

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("product", "TestProduct"))
        .and(query_param("include_fields", "id"))
        .and(query_param("limit", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1}, {"id": 2}, {"id": 3}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = count_run_action("count-json-test");

    let mut run_io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut run_io.writers(),
    )
    .await;

    assert!(result.is_ok(), "query run --count failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(run_io.out_str());
    assert_eq!(parsed["count"], 3);
}

#[tokio::test]
async fn query_run_count_table_prints_integer_only() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    let save_action = product_save_action("count-table-test", "TestProduct", 25);
    let mut save_io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &save_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut save_io.writers(),
    )
    .await;
    assert!(result.is_ok(), "query save failed: {result:?}");

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 10}, {"id": 11}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = count_run_action("count-table-test");

    let mut run_io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None),
        &mut run_io.writers(),
    )
    .await;

    assert!(result.is_ok(), "query run --count failed: {result:?}");
    assert_eq!(run_io.out_str().trim(), "2");
}

#[tokio::test]
async fn query_run_count_rejects_offset_and_paginate() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    for (offset, paginate) in [(Some(10), false), (None, true)] {
        let action = QueryAction::Run(RunArgs {
            page_args: crate::cli::PageArgs { offset, paginate },
            name: "count-conflict-test".into(),
            count: true,
            limit: None,
            fields: None,
            exclude_fields: None,
            server: None,
            created_since: None,
            changed_since: None,
            filters: QueryRunFilterArgs::default(),
            url: vec![],
            sort_args: crate::cli::SortArgs::default(),
        });
        let mut run_io = crate::test_helpers::CapturedIo::new();
        let result = crate::commands::query::execute(
            &action,
            &crate::commands::runtime::invocation::CommandContext::new(
                None,
                OutputFormat::Json,
                None,
            ),
            &mut run_io.writers(),
        )
        .await;

        assert!(
            matches!(result, Err(crate::error::BzrError::InputValidation { ref message, .. }) if message.contains("--count")),
            "expected count paging conflict, got {result:?}"
        );
    }

    let received = mock.received_requests().await.unwrap();
    assert!(
        received.is_empty(),
        "expected conflict validation before network I/O, got {} request(s)",
        received.len()
    );
}

#[tokio::test]
async fn query_run_count_ignores_saved_url_offset() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    update_config(|config| {
        config.queries.insert(
            "count-offset-test".into(),
            crate::types::SavedQuery {
                product: vec!["TestProduct".into()],
                raw_params: vec![("offset".into(), "50".into())],
                ..crate::types::SavedQuery::default()
            },
        );
        Ok(())
    })
    .unwrap();

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("product", "TestProduct"))
        .and(query_param("include_fields", "id"))
        .and(query_param("limit", "0"))
        .and(query_param_is_missing("offset"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1}, {"id": 2}, {"id": 3}, {"id": 4}]
        })))
        .expect(1)
        .mount(&mock)
        .await;

    let action = count_run_action("count-offset-test");

    let mut run_io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut run_io.writers(),
    )
    .await;

    assert!(result.is_ok(), "query run --count failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(run_io.out_str());
    assert_eq!(parsed["count"], 4);
}

#[tokio::test]
async fn query_run_with_limit_override() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    let save_action = product_save_action("override-test", "TestProduct", 100);
    let mut __io_a9 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &save_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
        count: false,
        limit: Some(5),
        fields: None,
        exclude_fields: None,
        server: None,
        created_since: None,
        changed_since: None,
        filters: QueryRunFilterArgs {
            whiteboard: vec![],
            target_milestone: vec![],
            version: vec![],
            op_sys: vec![],
            platform: vec![],
            resolution: vec![],
            qa_contact: vec![],
        },
        url: vec![],
        sort_args: crate::cli::SortArgs::default(),
    });
    let mut __io_a10 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &run_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a10.writers(),
    )
    .await;
    let _ = __io_a10.out_str().to_string();
    assert!(result.is_ok(), "query run with override failed: {result:?}");
}

#[tokio::test]
async fn query_run_applies_field_overrides() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    let save_action = QueryAction::Save(SaveArgs {
        name: "fields-test".into(),
        from_url: None,
        search: None,
        filters: BugFilterArgs {
            product: vec!["TestProduct".into()],
            component: vec![],
            status: vec![],
            priority: vec![],
            severity: vec![],
            whiteboard: vec![],
            target_milestone: vec![],
            version: vec![],
            op_sys: vec![],
            platform: vec![],
            resolution: vec![],
            qa_contact: vec![],
            url: vec![],
        },
        actor_filters: BugActorFilterArgs {
            assignee: vec![],
            creator: vec![],
        },
        limit: Some(10),
        fields: Some("id,status".into()),
        exclude_fields: Some("cc".into()),
        created_since: None,
        changed_since: None,

        sort_args: crate::cli::SortArgs::default(),
    });
    let mut __io_a13 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &save_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
        count: false,
        limit: None,
        fields: Some("id,summary".into()),
        exclude_fields: Some("comments".into()),
        server: None,
        created_since: None,
        changed_since: None,
        filters: QueryRunFilterArgs {
            whiteboard: vec![],
            target_milestone: vec![],
            version: vec![],
            op_sys: vec![],
            platform: vec![],
            resolution: vec![],
            qa_contact: vec![],
        },
        url: vec![],
        sort_args: crate::cli::SortArgs::default(),
    });
    let mut __io_a14 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &run_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
async fn query_run_with_server_override() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // Save a query that records a different server than the mock
    let save_action = save_action("server-test");
    let mut __io_a15 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &save_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a15.writers(),
    )
    .await;
    let _ = __io_a15.out_str().to_string();
    assert!(result.is_ok());

    // Patch the saved query to have a different server
    update_config(|config| {
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
        count: false,
        limit: None,
        fields: None,
        exclude_fields: None,
        server: Some("test".into()),
        created_since: None,
        changed_since: None,
        filters: QueryRunFilterArgs {
            whiteboard: vec![],
            target_milestone: vec![],
            version: vec![],
            op_sys: vec![],
            platform: vec![],
            resolution: vec![],
            qa_contact: vec![],
        },
        url: vec![],
        sort_args: crate::cli::SortArgs::default(),
    });
    let mut __io_a16 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &run_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
async fn query_run_rejects_malformed_created_since_override() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    // Pre-seed a saved query so the not-found branch doesn't fire first.
    update_config(move |c| {
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
        count: false,
        limit: None,
        fields: None,
        exclude_fields: None,
        server: None,
        created_since: Some("not-a-date".into()),
        changed_since: None,
        filters: QueryRunFilterArgs {
            whiteboard: vec![],
            target_milestone: vec![],
            version: vec![],
            op_sys: vec![],
            platform: vec![],
            resolution: vec![],
            qa_contact: vec![],
        },
        url: vec![],
        sort_args: crate::cli::SortArgs::default(),
    });
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await;
    let err = result.unwrap_err();
    assert_eq!(err.exit_code(), 7);
    assert!(err.to_string().contains("--created-since"));
}

#[tokio::test]
async fn query_run_overrides_replace_saved_field_filters() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    // Save a query with whiteboard=original and resolution=FIXED.
    let save_action = QueryAction::Save(SaveArgs {
        name: "field-override-test".into(),
        from_url: None,
        search: None,
        filters: BugFilterArgs {
            product: vec![],
            component: vec![],
            status: vec![],
            priority: vec![],
            severity: vec![],
            whiteboard: vec!["original".into()],
            target_milestone: vec![],
            version: vec![],
            op_sys: vec![],
            platform: vec![],
            resolution: vec!["FIXED".into()],
            qa_contact: vec![],
            url: vec![],
        },
        actor_filters: BugActorFilterArgs {
            assignee: vec![],
            creator: vec![],
        },
        limit: None,
        fields: None,
        exclude_fields: None,
        created_since: None,
        changed_since: None,

        sort_args: crate::cli::SortArgs::default(),
    });
    let mut __io_a20 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &save_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
        count: false,
        limit: None,
        fields: None,
        exclude_fields: None,
        server: None,
        created_since: None,
        changed_since: None,
        filters: QueryRunFilterArgs {
            whiteboard: vec!["overridden".into()],
            target_milestone: vec![],
            version: vec![],
            op_sys: vec![],
            platform: vec![],
            resolution: vec!["WONTFIX".into()],
            qa_contact: vec![],
        },
        url: vec![],
        sort_args: crate::cli::SortArgs::default(),
    });
    let mut __io_a21 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &run_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
        filters: BugFilterArgs {
            product: vec![],
            component: vec![],
            status: vec![],
            priority: vec![],
            severity: vec![],
            whiteboard: vec!["original".into()],
            target_milestone: vec![],
            version: vec![],
            op_sys: vec![],
            platform: vec![],
            resolution: vec![],
            qa_contact: vec![],
            url: vec![],
        },
        actor_filters: BugActorFilterArgs {
            assignee: vec![],
            creator: vec![],
        },
        limit: None,
        fields: None,
        exclude_fields: None,
        created_since: None,
        changed_since: None,

        sort_args: crate::cli::SortArgs::default(),
    });
    let mut __io_a22 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &save_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = crate::commands::query::execute(
        &run_action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
    crate::commands::query::execute(
        &save,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = crate::commands::query::execute(
        &run,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "run with default order should succeed: {result:?}"
    );
}

#[tokio::test]
async fn query_run_sort_override_takes_precedence() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    let save = product_save_action("order-override", "TestProduct", 10);
    crate::commands::query::execute(
        &save,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
    let result = crate::commands::query::execute(
        &run,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "run with --sort override should succeed: {result:?}"
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

#[tokio::test]
async fn query_run_applies_default_order_even_when_raw_params_present() {
    // Kill the `== with !=` mutant at run.rs:76 (inside the `.any(|(k, _)| k
    // == "order")` closure). When mutated to `!=`, `.any()` returns true for
    // any non-"order" key, flipping `!any(..)` to false and suppressing the
    // default `bug_id` order.  This test seeds a saved query with a raw
    // non-"order" param and asserts the default order is still applied.
    let (_lock, mock, _tmp) = setup_test_env().await;

    // Directly insert a saved query that carries a raw_param that is NOT
    // "order" (so it exercises the `k == "order"` check), has no structured
    // `order`, and has no explicit sort_args set.
    update_config(|config| {
        config.queries.insert(
            "raw-param-test".into(),
            crate::types::SavedQuery {
                product: vec!["TestProduct".into()],
                // A raw param whose key is NOT "order" — exercises the
                // `k == "order"` guard on the right side of the `&&`.
                raw_params: vec![("classification".into(), "MyClass".into())],
                ..crate::types::SavedQuery::default()
            },
        );
        Ok(())
    })
    .unwrap();

    // The wire must carry `order=bug_id` (the default), not be missing it.
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("order", "bug_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    run_q(&run_action("raw-param-test")).await.unwrap();
}
