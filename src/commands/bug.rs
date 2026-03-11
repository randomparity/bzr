use crate::cli::BugAction;
use crate::client::{BugzillaClient, CreateBugParams, SearchParams, UpdateBugParams};
use crate::config::Config;
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn execute(
    action: &BugAction,
    server: Option<&str>,
    format: &OutputFormat,
) -> Result<()> {
    let config = Config::load()?;
    let srv = config.active_server(server)?;
    let client = BugzillaClient::new(&srv.url, &srv.api_key)?;

    match action {
        BugAction::List {
            product,
            component,
            status,
            assignee,
            limit,
        } => {
            let params = SearchParams {
                product: product.clone(),
                component: component.clone(),
                status: status.clone(),
                assigned_to: assignee.clone(),
                limit: Some(*limit),
                ..Default::default()
            };
            let bugs = client.search_bugs(&params).await?;
            output::print_bugs(&bugs, format)?;
        }
        BugAction::View { id } => {
            let bug = client.get_bug(*id).await?;
            output::print_bug_detail(&bug, format)?;
        }
        BugAction::Search { query, limit } => {
            let params = SearchParams {
                quicksearch: Some(query.clone()),
                limit: Some(*limit),
                ..Default::default()
            };
            let bugs = client.search_bugs(&params).await?;
            output::print_bugs(&bugs, format)?;
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
            println!("Created bug #{}", id);
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
        } => {
            let params = UpdateBugParams {
                status: status.clone(),
                resolution: resolution.clone(),
                assigned_to: assignee.clone(),
                priority: priority.clone(),
                severity: severity.clone(),
                summary: summary.clone(),
                whiteboard: whiteboard.clone(),
            };
            client.update_bug(*id, &params).await?;
            println!("Updated bug #{}", id);
        }
    }
    Ok(())
}
