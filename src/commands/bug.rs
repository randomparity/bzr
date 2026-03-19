use crate::cli::BugAction;
use crate::config::ApiMode;
use crate::error::Result;
use crate::output;
use crate::types::OutputFormat;
use crate::types::{CreateBugParams, SearchParams, UpdateBugParams};

#[expect(
    clippy::too_many_lines,
    reason = "single match dispatch over many action variants"
)]
pub async fn execute(
    action: &BugAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_client(server, api).await?;

    match action {
        BugAction::List {
            product,
            component,
            status,
            assignee,
            creator,
            priority,
            severity,
            id,
            alias,
            limit,
            fields,
            exclude_fields,
        } => {
            let params = SearchParams {
                product: product.clone(),
                component: component.clone(),
                status: status.clone(),
                assigned_to: assignee.clone(),
                creator: creator.clone(),
                priority: priority.clone(),
                severity: severity.clone(),
                id: id.clone(),
                alias: alias.clone(),
                limit: Some(*limit),
                include_fields: fields.clone(),
                exclude_fields: exclude_fields.clone(),
                ..Default::default()
            };
            let bugs = client.search_bugs(&params).await?;
            output::print_bugs(&bugs, format);
        }
        BugAction::View {
            id,
            fields,
            exclude_fields,
        } => {
            let bug = client
                .get_bug(id, fields.as_deref(), exclude_fields.as_deref())
                .await?;
            output::print_bug_detail(&bug, format);
        }
        BugAction::History { id, since } => {
            let history = client.get_bug_history_since(*id, since.as_deref()).await?;
            if history.is_empty() {
                #[expect(clippy::print_stdout)]
                {
                    println!("No history for bug #{id}.");
                }
            } else {
                output::print_history(&history, format);
            }
        }
        BugAction::Search {
            query,
            limit,
            fields,
            exclude_fields,
        } => {
            let params = SearchParams {
                quicksearch: Some(query.clone()),
                limit: Some(*limit),
                include_fields: fields.clone(),
                exclude_fields: exclude_fields.clone(),
                ..Default::default()
            };
            let bugs = client.search_bugs(&params).await?;
            output::print_bugs(&bugs, format);
        }
        BugAction::Create {
            product,
            component,
            summary,
            version,
            description,
            priority,
            severity,
            assignee,
        } => {
            let params = CreateBugParams {
                product: product.clone(),
                component: component.clone(),
                summary: summary.clone(),
                version: version.clone(),
                description: description.clone(),
                priority: priority.clone(),
                severity: severity.clone(),
                assigned_to: assignee.clone(),
            };
            let id = client.create_bug(&params).await?;
            output::print_result(
                &serde_json::json!({"id": id, "resource": "bug", "action": "created"}),
                &format!("Created bug #{id}"),
                format,
            );
        }
        BugAction::Update {
            id,
            status,
            resolution,
            assignee,
            priority,
            severity,
            summary,
            whiteboard,
            flag,
        } => {
            let flags = super::shared::parse_flags(flag)?;
            let params = UpdateBugParams {
                status: status.clone(),
                resolution: resolution.clone(),
                assigned_to: assignee.clone(),
                priority: priority.clone(),
                severity: severity.clone(),
                summary: summary.clone(),
                whiteboard: whiteboard.clone(),
                flags,
            };
            client.update_bug(*id, &params).await?;
            output::print_result(
                &serde_json::json!({"id": id, "resource": "bug", "action": "updated"}),
                &format!("Updated bug #{id}"),
                format,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::await_holding_lock)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::test_helpers::{setup_config, ENV_LOCK};
    use crate::cli::BugAction;
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn bug_list_returns_bugs() {
        let _lock = ENV_LOCK.lock().unwrap();
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        Mock::given(method("GET"))
            .and(path("/rest/bug"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 1,
                    "summary": "Test bug",
                    "status": "NEW",
                    "resolution": "",
                    "assigned_to": "nobody@test.com",
                    "priority": "P1",
                    "severity": "normal",
                    "product": "TestProduct",
                    "component": "General",
                    "creation_time": "2025-01-01T00:00:00Z",
                    "last_change_time": "2025-01-01T00:00:00Z"
                }]
            })))
            .mount(&mock)
            .await;

        let action = BugAction::List {
            product: None,
            component: None,
            status: None,
            assignee: None,
            creator: None,
            priority: None,
            severity: None,
            id: vec![],
            alias: None,
            limit: 50,
            fields: None,
            exclude_fields: None,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn bug_view_returns_detail() {
        let _lock = ENV_LOCK.lock().unwrap();
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        Mock::given(method("GET"))
            .and(path("/rest/bug/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": [{
                    "id": 42,
                    "summary": "Test bug",
                    "status": "NEW",
                    "resolution": "",
                    "assigned_to": "nobody@test.com",
                    "priority": "P1",
                    "severity": "normal",
                    "product": "TestProduct",
                    "component": "General",
                    "creation_time": "2025-01-01T00:00:00Z",
                    "last_change_time": "2025-01-01T00:00:00Z"
                }]
            })))
            .mount(&mock)
            .await;

        let action = BugAction::View {
            id: "42".to_string(),
            fields: None,
            exclude_fields: None,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_ok());
    }
}
