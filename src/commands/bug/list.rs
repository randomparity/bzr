use crate::cli::{BugAction, FieldArgs};
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::resources::bug::{canonical_field_list, write_bugs, ColumnSpec};
use crate::output::writers::Writers;
use crate::types::{OutputFormat, SearchParams};
use crate::validation::parse_optional_date;

pub(super) async fn handle(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
    w: &mut Writers<'_>,
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
        field_args: FieldArgs {
            fields,
            exclude_fields,
        },
        created_since,
        changed_since,
        whiteboard,
        target_milestone,
        version,
        op_sys,
        platform,
        resolution,
        qa_contact,
        url,
        sort_args,
        count,
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
        include_fields: canonical_field_list(fields.as_deref()),
        exclude_fields: canonical_field_list(exclude_fields.as_deref()),
        creation_time,
        last_change_time,
        whiteboard: whiteboard.clone(),
        target_milestone: target_milestone.clone(),
        version: version.clone(),
        op_sys: op_sys.clone(),
        platform: platform.clone(),
        resolution: resolution.clone(),
        qa_contact: qa_contact.clone(),
        url: url.clone(),
        order: Some(crate::validation::build_order(
            sort_args.sort.as_deref(),
            sort_args.order,
        )),
        ..Default::default()
    };
    if *count {
        let bugs = client
            .search_bugs(&super::count_search_params(params))
            .await?;
        crate::output::result_types::write_count(bugs.len(), format, w.out);
        return Ok(());
    }

    let spec = ColumnSpec::new(fields.as_deref(), exclude_fields.as_deref());
    let bugs = client.search_bugs(&params).await?;
    write_bugs(&bugs, spec, format, w.out, w.err);
    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
