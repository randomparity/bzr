#![expect(clippy::unwrap_used)]

use std::path::PathBuf;

use crate::cli::{
    BugActorFilterArgs, BugFilterArgs, QueryAction, QueryUpdateArgs, SaveArgs, ShowArgs,
};
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

fn empty_update(name: &str) -> QueryAction {
    QueryAction::Update(QueryUpdateArgs {
        name: name.into(),
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
        created_since: None,
        changed_since: None,

        clear: vec![],
        sort_args: crate::cli::SortArgs::default(),
    })
}

async fn run_q(action: &QueryAction) -> crate::error::Result<()> {
    let mut io = crate::test_helpers::CapturedIo::new();
    crate::commands::query::execute(
        action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut io.writers(),
    )
    .await
}

async fn run_action_output(action: &QueryAction, format: OutputFormat) -> String {
    let mut io = crate::test_helpers::CapturedIo::new();
    crate::commands::query::execute(
        action,
        &crate::commands::runtime::context::CommandContext::new(None, format, None),
        &mut io.writers(),
    )
    .await
    .unwrap();
    io.out_str().to_string()
}

async fn show_query_json(name: &str) -> serde_json::Value {
    let output = run_action_output(
        &QueryAction::Show(ShowArgs { name: name.into() }),
        OutputFormat::Json,
    )
    .await;
    serde_json::from_str(output.trim()).unwrap()
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
async fn query_update_replaces_filter_keeps_rest() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap(); // product=Firefox, status=NEW, limit=25

    let mut a = empty_update("q");
    if let QueryAction::Update(QueryUpdateArgs { filters, .. }) = &mut a {
        filters.status = vec!["ASSIGNED".into()];
    }
    run_q(&a).await.unwrap();

    let config = load_config();
    let q = &config.queries["q"];
    assert_eq!(q.status, vec!["ASSIGNED".to_string()]);
    assert_eq!(q.product, vec!["Firefox".to_string()]); // untouched
    assert_eq!(q.limit, Some(25)); // untouched
}

#[tokio::test]
async fn query_update_search_reports_effective_search_kind() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap();

    let mut update = empty_update("q");
    if let QueryAction::Update(QueryUpdateArgs { search, .. }) = &mut update {
        *search = Some("crash in tab".into());
    }
    run_q(&update).await.unwrap();

    let shown = show_query_json("q").await;
    assert_eq!(shown["kind"], "search");

    let listed = run_action_output(&QueryAction::List, OutputFormat::Table).await;
    assert!(
        listed.contains("q (kind=search"),
        "expected query list to report search kind, got: {listed:?}"
    );
}

#[tokio::test]
async fn query_update_clear_search_reports_effective_list_kind() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&empty_save_action("q", Some("crash in tab".into())))
        .await
        .unwrap();

    let mut add_filter = empty_update("q");
    if let QueryAction::Update(QueryUpdateArgs { filters, .. }) = &mut add_filter {
        filters.product = vec!["Firefox".into()];
    }
    run_q(&add_filter).await.unwrap();

    let mut clear_search = empty_update("q");
    if let QueryAction::Update(QueryUpdateArgs { clear, .. }) = &mut clear_search {
        *clear = vec!["search".into()];
    }
    run_q(&clear_search).await.unwrap();

    let shown = show_query_json("q").await;
    assert_eq!(shown["kind"], "list");

    let listed = run_action_output(&QueryAction::List, OutputFormat::Table).await;
    assert!(
        listed.contains("q (kind=list"),
        "expected query list to report list kind, got: {listed:?}"
    );
}

#[tokio::test]
async fn query_update_replaces_limit() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap();

    let mut a = empty_update("q");
    if let QueryAction::Update(QueryUpdateArgs { limit, .. }) = &mut a {
        *limit = Some(100);
    }
    run_q(&a).await.unwrap();

    assert_eq!(load_config().queries["q"].limit, Some(100));
}

#[tokio::test]
async fn query_update_clear_resets_filter() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap();

    let mut a = empty_update("q");
    if let QueryAction::Update(QueryUpdateArgs { clear, .. }) = &mut a {
        *clear = vec!["status".into()];
    }
    run_q(&a).await.unwrap();

    let config = load_config();
    assert!(config.queries["q"].status.is_empty());
    assert_eq!(config.queries["q"].product, vec!["Firefox".to_string()]);
}

#[tokio::test]
async fn query_update_unknown_query_errors() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    let mut a = empty_update("missing");
    if let QueryAction::Update(QueryUpdateArgs { filters, .. }) = &mut a {
        filters.status = vec!["NEW".into()];
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
    if let QueryAction::Update(QueryUpdateArgs { clear, .. }) = &mut a {
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
    if let QueryAction::Update(QueryUpdateArgs { clear, .. }) = &mut a {
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
    if let QueryAction::Update(QueryUpdateArgs { created_since, .. }) = &mut a {
        *created_since = Some("not-a-date".into());
    }
    assert!(run_q(&a).await.is_err());
}

#[tokio::test]
async fn query_update_clear_wins_over_set() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap(); // product=Firefox, status=NEW

    let mut a = empty_update("q");
    if let QueryAction::Update(QueryUpdateArgs { filters, clear, .. }) = &mut a {
        filters.status = vec!["ASSIGNED".into()];
        *clear = vec!["status".into()];
    }
    run_q(&a).await.unwrap();
    // status was both set and cleared -> cleared.
    assert!(load_config().queries["q"].status.is_empty());
}

#[tokio::test]
async fn query_update_sets_dates_and_sort() {
    let (_lock, _mock, _tmp) = setup_test_env().await;
    run_q(&save_action("q")).await.unwrap();

    let mut a = empty_update("q");
    if let QueryAction::Update(QueryUpdateArgs {
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

    let config = load_config();
    let q = &config.queries["q"];
    assert!(q.creation_time.is_some());
    assert!(q.last_change_time.is_some());
    assert!(q.order.is_some());
}

#[tokio::test]
async fn query_update_from_url_replaces_existing_query() {
    let (_lock, mock, _tmp) = setup_test_env().await;
    let original_url = format!(
        "{}/buglist.cgi?product=OldProduct&bug_status=NEW&f1=old&o1=substring&v1=stale&limit=25",
        mock.uri()
    );
    run_q(&url_save_action("web", original_url)).await.unwrap();

    let refreshed_url = format!(
        "{}/buglist.cgi?product=NewProduct&bug_status=ASSIGNED&priority=P1\
         &f1=qa_contact&o1=changedfrom&v1=qa%40example.com&limit=5&order=Bug+Number",
        mock.uri()
    );
    let mut update = empty_update("web");
    if let QueryAction::Update(QueryUpdateArgs {
        from_url,
        limit,
        fields,
        exclude_fields,
        created_since,
        changed_since,
        sort_args,
        ..
    }) = &mut update
    {
        *from_url = Some(refreshed_url);
        *limit = Some(77);
        *fields = Some("id,summary".into());
        *exclude_fields = Some("creator".into());
        *created_since = Some("2026-04-01".into());
        *changed_since = Some("2026-04-15T12:00:00Z".into());
        sort_args.sort = Some("priority".into());
    }

    run_q(&update).await.unwrap();

    let config = load_config();
    let q = &config.queries["web"];
    assert_eq!(q.kind(), crate::types::QueryKind::Url);
    assert_eq!(q.product, vec!["NewProduct"]);
    assert_eq!(q.status, vec!["ASSIGNED"]);
    assert_eq!(q.priority, vec!["P1"]);
    assert_eq!(q.limit, Some(77));
    assert_eq!(q.fields.as_deref(), Some("id,summary"));
    assert_eq!(q.exclude_fields.as_deref(), Some("creator"));
    assert_eq!(q.creation_time.as_deref(), Some("2026-04-01T00:00:00Z"));
    assert_eq!(q.last_change_time.as_deref(), Some("2026-04-15T12:00:00Z"));
    assert_eq!(q.order.as_deref(), Some("priority ASC, bug_id"));
    assert_eq!(q.server.as_deref(), Some("test"));
    assert!(q
        .source_url
        .as_deref()
        .is_some_and(|url| url.contains("product=NewProduct")));
    assert!(q
        .raw_params
        .contains(&("f1".to_string(), "qa_contact".to_string())));
    assert!(q
        .raw_params
        .contains(&("order".to_string(), "Bug Number".to_string())));
    assert!(!q
        .raw_params
        .contains(&("v1".to_string(), "stale".to_string())));
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
    if let QueryAction::Update(QueryUpdateArgs { filters, .. }) = &mut a {
        filters.product = vec!["Thunderbird".into()];
    }
    run_q(&a).await.unwrap(); // must NOT fail with "no changes"

    let config = load_config();
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
    if let QueryAction::Update(QueryUpdateArgs { filters, .. }) = &mut a {
        filters.component = vec!["General".into()];
    }
    run_q(&a).await.unwrap();

    let config = load_config();
    assert_eq!(
        config.queries["q"].component,
        vec!["General".to_string()],
        "a sole --component update must be applied"
    );
}
