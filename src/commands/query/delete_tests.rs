#![expect(clippy::unwrap_used)]

use crate::cli::{BugActorFilterArgs, BugFilterArgs, DeleteArgs, QueryAction, SaveArgs, ShowArgs};
use crate::test_helpers::setup_test_env;
use crate::types::OutputFormat;

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

#[tokio::test]
async fn query_delete_unknown_errors() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let action = QueryAction::Delete(DeleteArgs {
        name: "nonexistent".into(),
    });
    let result = crate::commands::query::execute(
        &action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
async fn query_delete_removes_saved_query() {
    let mut __cap_io = crate::test_helpers::CapturedIo::new();
    let (_lock, _mock, _tmp) = setup_test_env().await;

    let save_action = product_save_action("delete-me", "Firefox", 1);
    let mut __io_a12 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &save_action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __io_a12.writers(),
    )
    .await;
    let _ = __io_a12.out_str().to_string();
    assert!(result.is_ok());

    let delete_action = QueryAction::Delete(DeleteArgs {
        name: "delete-me".into(),
    });
    let mut __io5 = crate::test_helpers::CapturedIo::new();
    let result = crate::commands::query::execute(
        &delete_action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
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
    let err = crate::commands::query::execute(
        &show_action,
        &crate::commands::runtime::context::CommandContext::new(None, OutputFormat::Json, None),
        &mut __cap_io.writers(),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("not found"));
}
