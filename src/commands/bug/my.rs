use crate::cli::{BugAction, FieldArgs};
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::resources::bug::{canonical_field_list, write_bugs, ColumnSpec};
use crate::output::writers::Writers;
use crate::types::{OutputFormat, SearchParams};

pub(super) async fn handle(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let BugAction::My {
        created,
        cc,
        all,
        status,
        limit,
        field_args: FieldArgs {
            fields,
            exclude_fields,
        },
        sort_args,
        count,
    } = action
    else {
        unreachable!()
    };

    let spec = ColumnSpec::new(fields.as_deref(), exclude_fields.as_deref());

    let whoami = client.whoami().await?;
    let email = whoami.name;
    let mut all_bugs: Vec<crate::types::Bug> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    // Build search params for each enabled filter, varying one field.
    let mut base = SearchParams {
        status: status.clone(),
        limit: Some(*limit),
        include_fields: canonical_field_list(fields.as_deref()),
        exclude_fields: canonical_field_list(exclude_fields.as_deref()),
        order: Some(crate::validation::build_order(
            sort_args.sort.as_deref(),
            sort_args.order,
        )),
        ..Default::default()
    };
    // `--count` needs every distinct match, so fetch IDs only and lift the
    // per-category limit; the dedup below then yields the true distinct count.
    if *count {
        base = super::count_search_params(base);
    }
    let mut searches = Vec::new();
    if *all || (!created && !cc) {
        let mut p = base.clone();
        p.assigned_to = vec![email.clone()];
        searches.push(p);
    }
    if *all || *created {
        let mut p = base.clone();
        p.creator = vec![email.clone()];
        searches.push(p);
    }
    if *all || *cc {
        let mut p = base;
        p.cc = Some(email.clone());
        searches.push(p);
    }

    for params in &searches {
        for bug in client.search_bugs(params).await? {
            // When counting, only the deduped id set matters — don't retain rows.
            if seen_ids.insert(bug.id) && !*count {
                all_bugs.push(bug);
            }
        }
    }

    if *count {
        crate::output::result_types::write_count(seen_ids.len(), format, w.out);
        return Ok(());
    }

    write_bugs(&all_bugs, spec, format, w.out, w.err);
    Ok(())
}

#[cfg(test)]
#[path = "my_tests.rs"]
mod tests;
