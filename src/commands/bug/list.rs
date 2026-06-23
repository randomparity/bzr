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
        filters,
        actor_filters,
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
        sort_args,
        page_args: crate::cli::PageArgs { offset, paginate },
        count,
    } = args;

    super::ensure_no_paging_with_count(*count, *offset, *paginate)?;

    let creation_time = parse_optional_date(created_since.as_deref(), "--created-since")?;
    let last_change_time = parse_optional_date(changed_since.as_deref(), "--changed-since")?;

    let mut params = SearchParams {
        id: id.clone(),
        alias: alias.clone(),
        summary: summary.clone(),
        limit: Some(*limit),
        offset: *offset,
        include_fields: canonical_field_list(fields.as_deref()),
        exclude_fields: canonical_field_list(exclude_fields.as_deref()),
        creation_time,
        last_change_time,
        order: Some(crate::validation::build_order(
            sort_args.sort.as_deref(),
            sort_args.order,
        )),
        ..Default::default()
    };
    filters.write_search_filters(&mut params);
    actor_filters.write_search_filters(&mut params);
    if *count {
        let bugs = client
            .search_bugs(&super::count_search_params(params))
            .await?;
        crate::output::result_types::write_count(bugs.len(), format, w.out);
        return Ok(());
    }

    let spec = ColumnSpec::new(fields.as_deref(), exclude_fields.as_deref());
    let page = crate::commands::runtime::paging::fetch_page(client, &params, *paginate).await?;
    write_bugs(&page.bugs, spec, format, w.out, w.err);
    crate::commands::runtime::paging::write_truncation_note(
        &page,
        params.limit,
        *offset,
        format,
        w,
    );
    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
