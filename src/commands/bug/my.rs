use crate::cli::MyArgs;
use crate::client::BugzillaClient;
use crate::commands::runtime::search::fields::ColumnSpec;
use crate::commands::runtime::search::policy::{count_search_params, ensure_no_paging_with_count};
use crate::commands::runtime::search::setup::{
    build_base_search_params, write_bug_page, BaseSearchInputs, PageWindow,
};
use crate::error::{BzrError, Result};
use crate::output::writers::Writers;
use crate::types::bug::Bug;
use crate::types::output::OutputFormat;

pub(super) async fn handle(
    client: &BugzillaClient,
    args: &MyArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let offset = args.page_args.offset;
    let paginate = args.page_args.paginate;
    ensure_no_paging_with_count(args.count, offset, paginate)?;

    let fields = args.field_args.fields.as_deref();
    let exclude_fields = args.field_args.exclude_fields.as_deref();
    let spec = ColumnSpec::new(fields, exclude_fields);

    let whoami = client.whoami().await?;
    let email = whoami.name.or(whoami.login).ok_or_else(|| {
        BzrError::DataIntegrity("whoami response missing name/login field".into())
    })?;
    let mut all_bugs: Vec<Bug> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    let mut base = build_base_search_params(BaseSearchInputs {
        limit: args.limit,
        offset,
        fields,
        exclude_fields,
        created_since: args.created_since.as_deref(),
        changed_since: args.changed_since.as_deref(),
        sort: args.sort_args.sort.as_deref(),
        order: args.sort_args.order,
    })?;
    args.filters.write_search_filters(&mut base);
    // `--count` needs every distinct match, so fetch IDs only and lift the
    // per-category limit; the dedup below then yields the true distinct count.
    if args.count {
        base = count_search_params(base);
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
        // `bug my` is out of the `--progress` scope (issue #462): its
        // multi-category loop would emit several `done` events. Pass `None` so
        // it stays silent regardless of the flag.
        let page =
            crate::commands::runtime::search::paging::fetch_page(client, params, paginate, None, w)
                .await?;
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

    let page = crate::commands::runtime::search::paging::Page {
        bugs: all_bugs,
        truncated,
    };
    write_bug_page(
        &page,
        spec,
        PageWindow {
            limit: Some(args.limit),
            offset,
        },
        format,
        w,
    );
    Ok(())
}

#[cfg(test)]
#[path = "my_tests.rs"]
mod tests;
