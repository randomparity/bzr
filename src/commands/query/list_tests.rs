#![expect(clippy::unwrap_used)]

use std::path::PathBuf;

use crate::cli::{BugActorFilterArgs, BugFilterArgs, QueryAction, SaveArgs};
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

#[tokio::test]
async fn query_list_emits_saved_query_names() {
    // Saved queries must appear in `query list` output.
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let mut __io2 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::query::execute(
        &save_action("listed-query"),
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io2.writers(),
    )
    .await;

    let _ = __io2.out_str().to_string();
    result.unwrap();

    let mut __io3 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::query::execute(
        &QueryAction::List,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
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
async fn query_list_empty() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = QueryAction::List;
    let mut __io_a6 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a6.writers(),
    )
    .await;
    let _output = __io_a6.out_str().to_string();
    assert!(result.is_ok(), "query list failed: {result:?}");
}

#[tokio::test]
async fn query_list_table_sorts_entries_by_name() {
    let (_lock, _mock, _tmp) = setup_test_env().await;

    for name in ["zzz", "aaa"] {
        let mut __io6 = crate::test_helpers::CapturedIo::new();
        let result = crate::commands::query::execute(
            &save_action(name),
            &crate::commands::runtime::invocation::CommandContext::new(
                None,
                OutputFormat::Json,
                None,
            ),
            &mut __io6.writers(),
        )
        .await;
        let _ = __io6.out_str().to_string();
        assert!(result.is_ok());
    }

    let mut __io7 = crate::test_helpers::CapturedIo::new();

    let result = crate::commands::query::execute(
        &QueryAction::List,
        &crate::commands::runtime::invocation::CommandContext::new(None, OutputFormat::Table, None),
        &mut __io7.writers(),
    )
    .await;

    let _ = __io7.out_str().to_string();
    assert!(result.is_ok());

    let config = load_config();
    let mut names: Vec<&str> = config.queries.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["aaa", "zzz"]);
}
