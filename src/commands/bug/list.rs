use crate::cli::{FieldArgs, ListArgs};
use crate::client::BugzillaClient;
use crate::commands::runtime::search::fields::ColumnSpec;
use crate::commands::runtime::search::policy::{count_search_params, ensure_no_paging_with_count};
use crate::commands::runtime::search::setup::{
    build_base_search_params, write_bug_page, BaseSearchInputs, PageWindow,
};
use crate::error::Result;
use crate::output::writers::Writers;
use crate::types::output::{OutputFormat, ProgressFormat};

pub(super) async fn handle(
    client: &BugzillaClient,
    args: &ListArgs,
    format: OutputFormat,
    progress: Option<ProgressFormat>,
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

    ensure_no_paging_with_count(*count, *offset, *paginate)?;

    let mut params = build_base_search_params(BaseSearchInputs {
        limit: *limit,
        offset: *offset,
        fields: fields.as_deref(),
        exclude_fields: exclude_fields.as_deref(),
        created_since: created_since.as_deref(),
        changed_since: changed_since.as_deref(),
        sort: sort_args.sort.as_deref(),
        order: sort_args.order,
    })?;
    params.id.clone_from(id);
    params.alias.clone_from(alias);
    params.summary.clone_from(summary);
    filters.write_search_filters(&mut params);
    actor_filters.write_search_filters(&mut params);
    if *count {
        let bugs = client.search_bugs(&count_search_params(params)).await?;
        crate::output::result_types::write_count(bugs.len(), format, w.out);
        return Ok(());
    }

    let spec = ColumnSpec::new(fields.as_deref(), exclude_fields.as_deref());
    let page = crate::commands::runtime::search::paging::fetch_page(
        client, &params, *paginate, progress, w,
    )
    .await?;
    write_bug_page(
        &page,
        spec,
        PageWindow {
            limit: params.limit,
            offset: *offset,
        },
        format,
        w,
    );
    // The terminal `done` fires only after a fully successful paginate; `list`
    // has no fallible step past the write, so this is always reached on success.
    if *paginate {
        crate::output::progress::done_event(progress, w.err, page.bugs.len());
    }
    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
