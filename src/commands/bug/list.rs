use crate::cli::{FieldArgs, ListArgs};
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::resources::bug::{canonical_field_list, write_bugs, ColumnSpec};
use crate::output::writers::Writers;
use crate::types::{OutputFormat, SearchParams};
use crate::validation::parse_optional_date;

pub(super) async fn handle(
    client: &BugzillaClient,
    args: &ListArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let ListArgs {
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
        page_args: crate::cli::PageArgs { offset, paginate },
        count,
    } = args;

    super::ensure_no_paging_with_count(*count, *offset, *paginate)?;

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
        offset: *offset,
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
    let page = crate::commands::paging::fetch_page(client, &params, *paginate).await?;
    write_bugs(&page.bugs, spec, format, w.out, w.err);
    crate::commands::paging::write_truncation_note(&page, params.limit, *offset, format, w);
    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
