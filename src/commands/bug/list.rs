use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output;
use crate::types::{OutputFormat, SearchParams};
use crate::validation::parse_optional_date;

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
        created_since,
        changed_since,
    } = action
    else {
        unreachable!()
    };

    let creation_time = parse_optional_date(created_since.as_deref(), "--created-since")?;
    let last_change_time = parse_optional_date(changed_since.as_deref(), "--changed-since")?;

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
        creation_time,
        last_change_time,
        ..Default::default()
    };
    let bugs = client.search_bugs(&params).await?;
    output::print_bugs(&bugs, format);
    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
