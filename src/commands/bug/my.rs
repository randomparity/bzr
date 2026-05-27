use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::resources::bug::{
    canonical_field_list, validate_table_columns, write_bugs, ColumnSpec,
};
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
        fields,
        exclude_fields,
    } = action
    else {
        unreachable!()
    };

    let spec = ColumnSpec {
        include: fields.as_deref(),
        exclude: exclude_fields.as_deref(),
    };
    if format == OutputFormat::Table {
        validate_table_columns(spec)?;
    }

    let whoami = client.whoami().await?;
    let email = whoami.name;
    let mut all_bugs: Vec<crate::types::Bug> = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    // Build search params for each enabled filter, varying one field.
    let base = SearchParams {
        status: status.clone(),
        limit: Some(*limit),
        include_fields: canonical_field_list(fields.as_deref()),
        exclude_fields: canonical_field_list(exclude_fields.as_deref()),
        ..Default::default()
    };
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
            if seen_ids.insert(bug.id) {
                all_bugs.push(bug);
            }
        }
    }

    write_bugs(&all_bugs, spec, format, w.out, w.err);
    Ok(())
}

#[cfg(test)]
#[path = "my_tests.rs"]
mod tests;
