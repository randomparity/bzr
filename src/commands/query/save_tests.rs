#![expect(clippy::unwrap_used)]

use std::path::PathBuf;

use crate::cli::{BugActorFilterArgs, BugFilterArgs, QueryAction, SaveArgs, ShowArgs};
use crate::config::Config;
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

fn current_config_path() -> PathBuf {
    Config::path_at(None).unwrap()
}

fn load_config() -> Config {
    let path = current_config_path();
    Config::load_at(Some(&path)).unwrap()
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

fn empty_save_action(name: &str, search: Option<String>) -> QueryAction {
    QueryAction::Save(SaveArgs {
        name: name.into(),
        from_url: None,
        search,
        filters: BugFilterArgs {
            product: vec![],
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
        limit: None,
        fields: None,
        exclude_fields: None,
        created_since: None,
        changed_since: None,

        sort_args: crate::cli::SortArgs::default(),
    })
}

fn url_save_action(name: &str, url: String) -> QueryAction {
    QueryAction::Save(SaveArgs {
        name: name.into(),
        from_url: Some(url),
        search: None,
        filters: BugFilterArgs {
            product: vec![],
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
        limit: None,
        fields: None,
        exclude_fields: None,
        created_since: None,
        changed_since: None,

        sort_args: crate::cli::SortArgs::default(),
    })
}

#[tokio::test]
async fn query_save_and_show() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = save_action("test-q");
    let mut __io_a1 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a1.writers(),
    )
    .await;
    let _output = __io_a1.out_str().to_string();
    assert!(result.is_ok(), "query save failed: {result:?}");

    let action = QueryAction::Show(ShowArgs {
        name: "test-q".into(),
    });
    let mut __io_a2 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a2.writers(),
    )
    .await;
    let output = __io_a2.out_str().to_string();
    assert!(result.is_ok(), "query show failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
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
        filters: BugFilterArgs {
            product: vec!["Firefox".into()],
            component: vec!["General".into()],
            status: vec!["NEW".into()],
            priority: vec!["P1".into()],
            severity: vec!["major".into()],
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
            assignee: vec!["dev@test.com".into()],
            creator: vec!["reporter@test.com".into()],
        },
        limit: Some(7),
        fields: Some("id,summary".into()),
        exclude_fields: Some("comments".into()),
        created_since: None,
        changed_since: None,

        sort_args: crate::cli::SortArgs::default(),
    });
    let mut __io = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io.writers(),
    )
    .await;
    let _ = __io.out_str().to_string();
    result.unwrap();

    let action = QueryAction::Show(ShowArgs {
        name: "comprehensive".into(),
    });
    let mut __io_a3 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a3.writers(),
    )
    .await;
    let output = __io_a3.out_str().to_string();
    result.unwrap();
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
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
async fn query_save_search_kind() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = empty_save_action("crashes", Some("crash in tab".into()));
    let mut __io_a4 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a4.writers(),
    )
    .await;
    let _output = __io_a4.out_str().to_string();
    assert!(result.is_ok(), "query save failed: {result:?}");

    let action = QueryAction::Show(ShowArgs {
        name: "crashes".into(),
    });
    let mut __io_a5 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a5.writers(),
    )
    .await;
    let output = __io_a5.out_str().to_string();
    assert!(result.is_ok(), "query show failed: {result:?}");
    let parsed: serde_json::Value = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["kind"], "search");
    assert_eq!(parsed["quicksearch"], "crash in tab");
}

#[tokio::test]
async fn query_save_requires_filter() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = empty_save_action("empty", None);
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
async fn query_save_existing_entry_reports_updated() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let save_action = QueryAction::Save(SaveArgs {
        name: "existing".into(),
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
        limit: Some(10),
        fields: None,
        exclude_fields: None,
        created_since: None,
        changed_since: None,

        sort_args: crate::cli::SortArgs::default(),
    });
    let mut __io_a11 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &save_action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a11.writers(),
    )
    .await;
    let _ = __io_a11.out_str().to_string();
    assert!(result.is_ok());

    let update_action = QueryAction::Save(SaveArgs {
        name: "existing".into(),
        from_url: None,
        search: Some("updated".into()),
        filters: BugFilterArgs {
            product: vec![],
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
        limit: Some(5),
        fields: None,
        exclude_fields: None,
        created_since: None,
        changed_since: None,

        sort_args: crate::cli::SortArgs::default(),
    });
    let mut __io4 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &update_action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io4.writers(),
    )
    .await;
    let output = __io4.out_str().to_string();
    assert!(result.is_ok());

    let parsed = crate::test_helpers::json_envelope_data(&output);
    assert_eq!(parsed["name"], "existing");
    assert_eq!(parsed["action"], "updated");

    let config = load_config();
    let saved = &config.queries["existing"];
    assert_eq!(saved.quicksearch.as_deref(), Some("updated"));
    assert_eq!(saved.limit, Some(5));
    assert!(saved.product.is_empty());
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
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a17.writers(),
    )
    .await;
    let _output = __io_a17.out_str().to_string();
    assert!(result.is_ok(), "query save --from-url failed: {result:?}");

    let config = load_config();
    let saved = &config.queries["url-query"];
    assert_eq!(saved.kind(), crate::types::QueryKind::Url);
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
        filters: BugFilterArgs {
            product: vec!["Firefox".into()],
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
        limit: None,
        fields: None,
        exclude_fields: None,
        created_since: Some("garbage".into()),
        changed_since: None,

        sort_args: crate::cli::SortArgs::default(),
    });

    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
        filters: BugFilterArgs {
            product: vec!["Firefox".into()],
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
        limit: None,
        fields: None,
        exclude_fields: None,
        created_since: Some("2026-04-01".into()),
        changed_since: Some("2026-04-15T12:00:00Z".into()),

        sort_args: crate::cli::SortArgs::default(),
    });
    let mut __io8 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io8.writers(),
    )
    .await;
    let _ = __io8.out_str().to_string();
    result.unwrap();

    let cfg = load_config();
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
        filters: BugFilterArgs {
            product: vec![],
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
        limit: None,
        fields: None,
        exclude_fields: None,
        created_since: Some("2026-04-01".into()),
        changed_since: None,

        sort_args: crate::cli::SortArgs::default(),
    });
    let mut __io9 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io9.writers(),
    )
    .await;
    let _ = __io9.out_str().to_string();
    result.unwrap();
    let cfg = load_config();
    assert!(cfg.queries.contains_key("date-only"));
}

#[tokio::test]
async fn query_save_persists_158_field_filters() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = QueryAction::Save(SaveArgs {
        name: "field-filters".into(),
        from_url: None,
        search: None,
        filters: BugFilterArgs {
            product: vec![],
            component: vec![],
            status: vec![],
            priority: vec![],
            severity: vec![],
            whiteboard: vec!["needs-review".into()],
            target_milestone: vec!["5.0".into()],
            version: vec!["9.4".into()],
            op_sys: vec!["Linux".into()],
            platform: vec!["x86_64".into()],
            resolution: vec!["FIXED".into()],
            qa_contact: vec!["qa@example.com".into()],
            url: vec!["github.com/foo".into()],
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
    let mut __io_a18 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a18.writers(),
    )
    .await;
    let _output = __io_a18.out_str().to_string();
    assert!(result.is_ok(), "save failed: {result:?}");

    let cfg = load_config();
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
        filters: BugFilterArgs {
            product: vec![],
            component: vec![],
            status: vec![],
            priority: vec![],
            severity: vec![],
            whiteboard: vec!["wip".into()],
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
    let mut __io_a19 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
async fn query_save_persists_explicit_sort() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let mut save = product_save_action("order-persist", "TestProduct", 10);
    if let QueryAction::Save(SaveArgs { sort_args, .. }) = &mut save {
        sort_args.sort = Some("last_change_time".to_string());
        sort_args.order = crate::types::SortDirection::Desc;
    }
    crate::commands::query::execute(
        &save,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut crate::test_helpers::CapturedIo::new().writers(),
    )
    .await
    .unwrap();

    let config = load_config();
    let saved = config.queries.get("order-persist").unwrap();
    assert_eq!(
        saved.order.as_deref(),
        Some("last_change_time DESC, bug_id")
    );
}
