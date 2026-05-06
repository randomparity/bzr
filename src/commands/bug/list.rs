use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output;
use crate::types::{OutputFormat, SearchParams};

pub(super) async fn handle(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let BugAction::List {
        product,
        component,
        status,
        assignee,
        creator,
        priority,
        severity,
        id,
        alias,
        summary,
        limit,
        fields,
        exclude_fields,
    } = action
    else {
        unreachable!()
    };

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
        summary: summary.clone(),
        limit: Some(*limit),
        include_fields: fields.clone(),
        exclude_fields: exclude_fields.clone(),
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await?;
    output::print_bugs(&bugs, format);
    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
