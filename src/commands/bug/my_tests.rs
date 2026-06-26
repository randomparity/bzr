#![expect(clippy::expect_used)]

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
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
        ..Default::default()
    });
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    let output = __io.out_str().to_string();
    assert!(result.is_ok(), "bug my failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
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
        filters: crate::cli::BugFilterArgs {
            status: vec!["NEW".into()],
            ..Default::default()
        },
        limit: 7,
        field_args: crate::cli::FieldArgs {
            fields: Some("id,summary".into()),
            exclude_fields: Some("comments".into()),
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
        ..Default::default()
    });
    let mut __io2 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io2.writers(),
    )
    .await;
    let _ = __io2.out_str().to_string();
    assert!(result.is_ok(), "bug my with filters failed: {result:?}");
}

#[tokio::test]
async fn bug_my_all_passes_shared_filters_to_each_category() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_whoami(&mock).await;

    for identity_filter in ["assigned_to", "creator", "cc"] {
        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .and(query_param(identity_filter, "dev@test.com"))
            .and(query_param("product", "Core"))
            .and(query_param("component", "Networking"))
            .and(query_param("priority", "P1"))
            .and(query_param("severity", "S2"))
            .and(query_param("creation_time", "2026-04-01T00:00:00Z"))
            .and(query_param("last_change_time", "2026-04-15T12:00:00Z"))
            .and(query_param("whiteboard", "needs-review"))
            .and(query_param("target_milestone", "5.0"))
            .and(query_param("version", "9.4"))
            .and(query_param("op_sys", "Linux"))
            .and(query_param("platform", "x86_64"))
            .and(query_param("resolution", "FIXED"))
            .and(query_param("qa_contact", "qa@example.com"))
            .and(query_param("url", "github.com/foo"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
            .expect(1)
            .mount(&mock)
            .await;
    }

    let action = BugAction::My(crate::cli::MyArgs {
        page_args: crate::cli::PageArgs::default(),
        created: false,
        cc: false,
        all: true,
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
        filters: crate::cli::BugFilterArgs {
            product: vec!["Core".into()],
            component: vec!["Networking".into()],
            priority: vec!["P1".into()],
            severity: vec!["S2".into()],
            whiteboard: vec!["needs-review".into()],
            target_milestone: vec!["5.0".into()],
            version: vec!["9.4".into()],
            op_sys: vec!["Linux".into()],
            platform: vec!["x86_64".into()],
            resolution: vec!["FIXED".into()],
            qa_contact: vec!["qa@example.com".into()],
            url: vec!["github.com/foo".into()],
            ..Default::default()
        },
        created_since: Some("2026-04-01".into()),
        changed_since: Some("2026-04-15T12:00:00Z".into()),
    });
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(
        result.is_ok(),
        "bug my --all with shared filters failed: {result:?}"
    );
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
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
        ..Default::default()
    });
    let mut __io3 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
        ..Default::default()
    });
    let mut __io4 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
        ..Default::default()
    });
    let mut __io5 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io5.writers(),
    )
    .await;
    let output = __io5.out_str().to_string();
    assert!(result.is_ok(), "bug my --all failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    let bugs = parsed.as_array().expect("expected JSON array");
    assert_eq!(bugs.len(), 1, "duplicate bug should be deduplicated");
    assert_eq!(bugs[0]["id"], 42);
}

// ---- truncated |= page.truncated accumulator ----
// Kill the `replace |= with &=` mutant at my.rs:62.
// Scenario: `--all` runs three searches. The first category (assigned_to)
// is NOT truncated; the second (creator) IS truncated. With `|=` the
// accumulator stays true; with `&=` it would be reset to false because
// the first page had truncated=false, so `false &= true = false`.
#[tokio::test]
async fn bug_my_all_truncated_flag_set_when_any_category_is_truncated() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_whoami(&mock).await;

    // assigned_to search: limit=1 → probe asks limit=2 → return 1 bug → NOT truncated
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("assigned_to", "dev@test.com"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [{"id": 10, "summary": "A", "status": "NEW",
                      "product": "P", "component": "C"}]
        })))
        .mount(&mock)
        .await;

    // creator search: probe asks limit=2 → return 2 bugs → TRUNCATED
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("creator", "dev@test.com"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "bugs": [
                {"id": 20, "summary": "B", "status": "NEW", "product": "P", "component": "C"},
                {"id": 21, "summary": "C", "status": "NEW", "product": "P", "component": "C"}
            ]
        })))
        .mount(&mock)
        .await;

    // cc search: return 0 bugs → NOT truncated
    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("cc", "dev@test.com"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .mount(&mock)
        .await;

    let action = BugAction::My(crate::cli::MyArgs {
        page_args: crate::cli::PageArgs::default(),
        created: false,
        cc: false,
        all: true,
        limit: 1, // probe = limit+1 = 2; creator returns 2 → truncated
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
        ..Default::default()
    });

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        // JSON so truncation note goes to stderr (easy to assert).
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;

    assert!(
        result.is_ok(),
        "my --all truncation test failed: {result:?}"
    );
    // The truncation footer is only emitted when `truncated` is true.
    // With `|=` the accumulator becomes true when creator is truncated.
    // With `&=` the accumulator would stay false (first result was false),
    // so the note would be absent and this assertion would fail.
    assert!(
        io.err_str().contains("more available"),
        "truncation note expected in stderr but got:\n{}",
        io.err_str()
    );
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
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: true,
        ..Default::default()
    });
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    assert!(result.is_ok(), "my --all --count failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(__io.out_str());
    // Three searches each return ids {1,2}; deduped distinct count is 2.
    assert_eq!(parsed["count"], 2);
}

// ---- delete field offset from SearchParams in build_base_search_params ----
// Kill the `delete field offset` mutant at my.rs:97.
// With --offset, the `offset` query param must reach the server.
// A dropped field would leave offset=None → the param is absent → wiremock
// would not match the query_param("offset", "5") matcher and return 404-ish,
// but more importantly the mock with .expect(1) would not fire.
#[tokio::test]
async fn bug_my_offset_reaches_server() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_whoami(&mock).await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("assigned_to", "dev@test.com"))
        .and(query_param("offset", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::My(crate::cli::MyArgs {
        page_args: crate::cli::PageArgs {
            offset: Some(5),
            paginate: false,
        },
        created: false,
        cc: false,
        all: false,
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs::default(),
        count: false,
        ..Default::default()
    });

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok(), "my --offset failed: {result:?}");
}

// ---- delete field order from SearchParams in build_base_search_params ----
// Kill the `delete field order` mutant at my.rs:102.
// The `order` param is built from sort_args and must appear in the request.
// A dropped field → absent `order` → wiremock's query_param("order", …)
// matcher does not match → the .expect(1) mock fires 0 times → test fails.
#[tokio::test]
async fn bug_my_order_reaches_server() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    mount_whoami(&mock).await;

    Mock::given(method("GET"))
        .and(path("/rest/bug"))
        .and(query_param("assigned_to", "dev@test.com"))
        .and(query_param("order", "last_change_time DESC, bug_id"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})))
        .expect(1)
        .mount(&mock)
        .await;

    let action = BugAction::My(crate::cli::MyArgs {
        page_args: crate::cli::PageArgs::default(),
        created: false,
        cc: false,
        all: false,
        limit: 50,
        field_args: crate::cli::FieldArgs {
            fields: None,
            exclude_fields: None,
        },
        sort_args: crate::cli::SortArgs {
            sort: Some("last_change_time".into()),
            order: crate::types::SortDirection::Desc,
        },
        count: false,
        ..Default::default()
    });

    let mut io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::bug::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await;
    assert!(result.is_ok(), "my with --sort failed: {result:?}");
}
