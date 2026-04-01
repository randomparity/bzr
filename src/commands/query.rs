//! Saved query management commands.
//!
//! Query operations (save/list/show/delete) are pure local file I/O.
//! Only `run` requires a network client.

use crate::cli::QueryAction;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output;
use crate::types::{OutputFormat, QueryKind, SavedQuery};

pub async fn execute(
    action: &QueryAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<crate::types::ApiMode>,
) -> Result<()> {
    match action {
        QueryAction::Save { .. } => handle_save(action, format),
        QueryAction::List => handle_list(format),
        QueryAction::Show { .. } => handle_show(action, format),
        QueryAction::Delete { .. } => handle_delete(action, format),
        QueryAction::Run { .. } => handle_run(action, server, format, api).await,
    }
}

fn handle_save(action: &QueryAction, format: OutputFormat) -> Result<()> {
    let QueryAction::Save {
        name,
        search,
        product,
        component,
        status,
        assignee,
        creator,
        priority,
        severity,
        limit,
        fields,
        exclude_fields,
    } = action
    else {
        unreachable!()
    };

    let kind = if search.is_some() {
        QueryKind::Search
    } else {
        QueryKind::List
    };

    let query = SavedQuery {
        kind,
        product: product.clone(),
        component: component.clone(),
        status: status.clone(),
        assignee: assignee.clone(),
        creator: creator.clone(),
        priority: priority.clone(),
        severity: severity.clone(),
        quicksearch: search.clone(),
        limit: *limit,
        fields: fields.clone(),
        exclude_fields: exclude_fields.clone(),
    };

    if !query.has_filters() {
        return Err(BzrError::InputValidation(
            "query must have at least one filter set".into(),
        ));
    }

    let mut config = Config::load()?;
    let is_update = config.queries.contains_key(name.as_str());
    config.queries.insert(name.clone(), query);
    config.save()?;

    let verb = if is_update { "Updated" } else { "Saved" };
    output::print_query_saved(name, verb, format);
    Ok(())
}

fn handle_list(format: OutputFormat) -> Result<()> {
    let config = Config::load()?;
    output::print_query_list(&config.queries, format);
    Ok(())
}

fn handle_show(action: &QueryAction, format: OutputFormat) -> Result<()> {
    let QueryAction::Show { name } = action else {
        unreachable!()
    };
    let config = Config::load()?;
    let query = config
        .queries
        .get(name.as_str())
        .ok_or_else(|| BzrError::config(format!("query '{name}' not found")))?;
    output::print_query_detail(name, query, format);
    Ok(())
}

fn handle_delete(action: &QueryAction, format: OutputFormat) -> Result<()> {
    let QueryAction::Delete { name } = action else {
        unreachable!()
    };
    let mut config = Config::load()?;
    if config.queries.remove(name.as_str()).is_none() {
        return Err(BzrError::config(format!("query '{name}' not found")));
    }
    config.save()?;

    output::print_query_saved(name, "Deleted", format);
    Ok(())
}

async fn handle_run(
    action: &QueryAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<crate::types::ApiMode>,
) -> Result<()> {
    let QueryAction::Run {
        name,
        limit,
        fields,
        exclude_fields,
    } = action
    else {
        unreachable!()
    };

    let config = Config::load()?;
    let query = config
        .queries
        .get(name.as_str())
        .ok_or_else(|| BzrError::config(format!("query '{name}' not found")))?;

    let mut params = query.to_search_params();

    // Apply runtime overrides
    if let Some(limit) = limit {
        params.limit = Some(*limit);
    }
    if let Some(fields) = fields {
        params.include_fields = Some(fields.clone());
    }
    if let Some(exclude_fields) = exclude_fields {
        params.exclude_fields = Some(exclude_fields.clone());
    }

    let client = super::shared::connect_and_configure(server, api).await?;
    let bugs = client.search_bugs(&params).await?;
    output::print_bugs(&bugs, format);
    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use crate::cli::QueryAction;
    use crate::test_helpers::{capture_stdout, setup_test_env};
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn query_save_and_show() {
        let (_lock, _mock, _tmp) = setup_test_env().await;

        let action = QueryAction::Save {
            name: "test-q".into(),
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
        };
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
    async fn query_save_search_kind() {
        let (_lock, _mock, _tmp) = setup_test_env().await;

        let action = QueryAction::Save {
            name: "crashes".into(),
            search: Some("crash in tab".into()),
            product: vec![],
            component: vec![],
            status: vec![],
            assignee: vec![],
            creator: vec![],
            priority: vec![],
            severity: vec![],
            limit: Some(10),
            fields: None,
            exclude_fields: None,
        };
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

        let action = QueryAction::Save {
            name: "empty".into(),
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
        };
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
        let save_action = QueryAction::Save {
            name: "run-test".into(),
            search: None,
            product: vec!["TestProduct".into()],
            component: vec![],
            status: vec![],
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
        assert!(result.is_ok(), "query save failed: {result:?}");

        // Mock the bug search endpoint
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/rest/bug"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "bugs": [{
                        "id": 1,
                        "summary": "Test bug",
                        "status": "NEW",
                        "product": "TestProduct",
                        "component": "General"
                    }]
                })),
            )
            .mount(&mock)
            .await;

        // Run the saved query
        let run_action = QueryAction::Run {
            name: "run-test".into(),
            limit: None,
            fields: None,
            exclude_fields: None,
        };
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

        let save_action = QueryAction::Save {
            name: "override-test".into(),
            search: None,
            product: vec!["TestProduct".into()],
            component: vec![],
            status: vec![],
            assignee: vec![],
            creator: vec![],
            priority: vec![],
            severity: vec![],
            limit: Some(100),
            fields: None,
            exclude_fields: None,
        };
        let (result, _) =
            capture_stdout(super::execute(&save_action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());

        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/rest/bug"))
            .and(wiremock::matchers::query_param("limit", "5"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({"bugs": []})),
            )
            .expect(1)
            .mount(&mock)
            .await;

        let run_action = QueryAction::Run {
            name: "override-test".into(),
            limit: Some(5),
            fields: None,
            exclude_fields: None,
        };
        let (result, _) =
            capture_stdout(super::execute(&run_action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok(), "query run with override failed: {result:?}");
    }

    #[tokio::test]
    async fn query_run_unknown_errors() {
        let (_lock, _mock, _tmp) = setup_test_env().await;

        let action = QueryAction::Run {
            name: "nonexistent".into(),
            limit: None,
            fields: None,
            exclude_fields: None,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err(), "running unknown query should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("not found"),
            "expected not-found error, got: {err}"
        );
    }
}
