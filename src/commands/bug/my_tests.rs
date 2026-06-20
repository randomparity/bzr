#![expect(clippy::unwrap_used, clippy::expect_used)]

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::cli::BugAction;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

async fn mount_whoami(mock: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/rest/whoami"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "dev@test.com",
            "real_name": "Dev User",
            "id": 1
        })))
        .mount(mock)
        .await;
}

#[tokio::test]
async fn bug_my_returns_assigned_by_default() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    mount_whoami(&mock).await;

    // Mock assigned-to search
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 10,
                "summary": "Assigned bug",
                "status": "NEW",
                "assigned_to": "dev@test.com",
                "product": "TestProduct",
                "component": "General"
            }]
        })))
        .mount(&mock)
        .await;

    let action = BugAction::My(crate::cli::MyArgs {
        page_args: crate::cli::PageArgs::default(),
        created: false,
        cc: false,
        all: false,
        status: vec![],
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
    });
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut __io.writers())
            .await;
    let output = __io.out_str().to_string();
    assert!(result.is_ok(), "bug my failed: {result:?}");
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    assert_eq!(parsed[0]["id"], 10);
    assert_eq!(parsed[0]["summary"], "Assigned bug");
}

#[tokio::test]
async fn bug_my_passes_status_limit_and_field_filters() {
    // status / limit / fields / exclude_fields must reach the search.
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_whoami(&mock).await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("status", "NEW"))
        .and(query_param("limit", "8"))
        .and(query_param("include_fields", "id,summary"))
        .and(query_param("exclude_fields", "comments"))
        .and(query_param("assigned_to", "dev@test.com"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::My(crate::cli::MyArgs {
        page_args: crate::cli::PageArgs::default(),
        created: false,
        cc: false,
        all: false,
        status: vec!["NEW".into()],
        limit: 7,
        field_args: crate::cli::FieldArgs {
            fields: Some("id,summary".into()),
            exclude_fields: Some("comments".into()),
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
    });
    let mut __io2 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io2.writers(),
    )
    .await;
    let _ = __io2.out_str().to_string();
    assert!(result.is_ok(), "bug my with filters failed: {result:?}");
}

#[tokio::test]
async fn bug_my_created_only_runs_creator_search_not_assigned() {
    // `--created` (without `--all`) must search by `creator=`, NOT by
    // `assigned_to=` and NOT by `cc=`.
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_whoami(&mock).await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("creator", "dev@test.com"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::My(crate::cli::MyArgs {
        page_args: crate::cli::PageArgs::default(),
        created: true,
        cc: false,
        all: false,
        status: vec![],
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
    });
    let mut __io3 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io3.writers(),
    )
    .await;
    let _ = __io3.out_str().to_string();
    assert!(result.is_ok(), "bug my --created failed: {result:?}");
}

#[tokio::test]
async fn bug_my_cc_only_runs_cc_search_not_assigned_or_creator() {
    let (_lock, mock, _tmp) = setup_test_env().await;

    mount_whoami(&mock).await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("cc", "dev@test.com"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::My(crate::cli::MyArgs {
        page_args: crate::cli::PageArgs::default(),
        created: false,
        cc: true,
        all: false,
        status: vec![],
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
    });
    let mut __io4 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io4.writers(),
    )
    .await;
    let _ = __io4.out_str().to_string();
    assert!(result.is_ok(), "bug my --cc failed: {result:?}");
}

#[tokio::test]
async fn bug_my_all_deduplicates() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_whoami(&mock).await;

    // All three searches return the same bug — should appear only once
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{
                "id": 42,
                "summary": "Shared bug",
                "status": "NEW",
                "assigned_to": "dev@test.com",
                "product": "TestProduct",
                "component": "General"
            }]
        })))
        .mount(&mock)
        .await;

    let action = BugAction::My(crate::cli::MyArgs {
        page_args: crate::cli::PageArgs::default(),
        created: false,
        cc: false,
        all: true,
        status: vec![],
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
    });
    let mut __io5 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        None,
        OutputFormat::Json,
        None,
        &mut __io5.writers(),
    )
    .await;
    let output = __io5.out_str().to_string();
    assert!(result.is_ok(), "bug my --all failed: {result:?}");
    let parsed: serde_json::Value =
        serde_json::from_str::<serde_json::Value>(output.trim()).unwrap();
    let bugs = parsed.as_array().expect("expected JSON array");
    assert_eq!(bugs.len(), 1, "duplicate bug should be deduplicated");
    assert_eq!(bugs[0]["id"], 42);
}

#[tokio::test]
async fn bug_my_all_count_reports_distinct_total() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_whoami(&mock).await;
    // All three category searches return the same ids {1,2}; --count must
    // report the distinct total (2), not the sum (6), and must request
    // id-only fields with limit=0.
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("include_fields", "id"))
        .and(query_param("limit", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 1}, {"id": 2}]
        })))
        .mount(&mock)
        .await;

    let action = BugAction::My(crate::cli::MyArgs {
        page_args: crate::cli::PageArgs::default(),
        created: false,
        cc: false,
        all: true,
        status: vec![],
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: true,
    });
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result =
        crate::commands::bug::execute(&action, None, OutputFormat::Json, None, &mut __io.writers())
            .await;
    assert!(result.is_ok(), "my --all --count failed: {result:?}");
    let parsed: serde_json::Value = serde_json::from_str(__io.out_str().trim()).unwrap();
    // Three searches each return ids {1,2}; deduped distinct count is 2.
    assert_eq!(parsed["count"], 2);
}
