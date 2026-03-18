use crate::cli::BugAction;
use crate::client::{CreateBugParams, SearchParams, UpdateBugParams};
use crate::error::Result;
use crate::output::{self, OutputFormat};

#[expect(
    clippy::too_many_lines,
    reason = "single match dispatch over many action variants"
)]
pub async fn execute(action: &BugAction, server: Option<&str>, format: OutputFormat) -> Result<()> {
    let client = super::shared::build_client(server).await?;

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
                .get_bug_with_fields(id, fields.as_deref(), exclude_fields.as_deref())
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
