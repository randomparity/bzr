use crate::cli::MyArgs;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::resources::bug::{canonical_field_list, write_bugs, ColumnSpec};
use crate::output::writers::Writers;
use crate::types::{OutputFormat, SearchParams};
use crate::validation::parse_optional_date;

pub(super) async fn handle(
    client: &BugzillaClient,
    args: &MyArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let offset = args.page_args.offset;
    let paginate = args.page_args.paginate;
    super::ensure_no_paging_with_count(args.count, offset, paginate)?;

    let fields = args.field_args.fields.as_deref();
    let exclude_fields = args.field_args.exclude_fields.as_deref();
    let spec = ColumnSpec::new(fields, exclude_fields);

    let whoami = client.whoami().await?;
    let email = whoami.name;
    let mut all_bugs: Vec<crate::types::Bug> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    let mut base = build_base_search_params(args)?;
    // `--count` needs every distinct match, so fetch IDs only and lift the
    // per-category limit; the dedup below then yields the true distinct count.
    if args.count {
        base = super::count_search_params(base);
    }
    let mut searches = Vec::new();
    if args.all || (!args.created && !args.cc) {
        let mut p = base.clone();
        p.assigned_to = vec![email.clone()];
        searches.push(p);
    }
    if args.all || args.created {
        let mut p = base.clone();
        p.creator = vec![email.clone()];
        searches.push(p);
    }
    if args.all || args.cc {
        let mut p = base;
        p.cc = Some(email.clone());
        searches.push(p);
    }

    // Page each category independently (a single global offset can't span the
    // overlapping assigned/created/cc sets); the result is the deduped union.
    // `truncated` means at least one category had more rows than `--limit`.
    let mut truncated = false;
    for params in &searches {
        let page = crate::commands::runtime::paging::fetch_page(client, params, paginate).await?;
        truncated |= page.truncated;
        for bug in page.bugs {
            // When counting, only the deduped id set matters — don't retain rows.
            if seen_ids.insert(bug.id) && !args.count {
                all_bugs.push(bug);
            }
        }
    }

    if args.count {
        crate::output::result_types::write_count(seen_ids.len(), format, w.out);
        return Ok(());
    }

    write_bugs(&all_bugs, spec, format, w.out, w.err);
    let page = crate::commands::runtime::paging::Page {
        bugs: all_bugs,
        truncated,
    };
    crate::commands::runtime::paging::write_truncation_note(
        &page,
        Some(args.limit),
        offset,
        format,
        w,
    );
    Ok(())
}

fn build_base_search_params(args: &MyArgs) -> Result<SearchParams> {
    let creation_time = parse_optional_date(args.created_since.as_deref(), "--created-since")?;
    let last_change_time = parse_optional_date(args.changed_since.as_deref(), "--changed-since")?;

    Ok(SearchParams {
        product: args.product.clone(),
        component: args.component.clone(),
        status: args.status.clone(),
        priority: args.priority.clone(),
        severity: args.severity.clone(),
        limit: Some(args.limit),
        offset: args.page_args.offset,
        include_fields: canonical_field_list(args.field_args.fields.as_deref()),
        exclude_fields: canonical_field_list(args.field_args.exclude_fields.as_deref()),
        creation_time,
        last_change_time,
        whiteboard: args.whiteboard.clone(),
        target_milestone: args.target_milestone.clone(),
        version: args.version.clone(),
        op_sys: args.op_sys.clone(),
        platform: args.platform.clone(),
        resolution: args.resolution.clone(),
        qa_contact: args.qa_contact.clone(),
        url: args.url.clone(),
        order: Some(crate::validation::build_order(
            args.sort_args.sort.as_deref(),
            args.sort_args.order,
        )),
        ..Default::default()
    })
}

#[cfg(test)]
#[path = "my_tests.rs"]
mod tests;
