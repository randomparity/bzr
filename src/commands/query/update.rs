use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::shared::merge_set;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::resources::query::write_query_saved;
use crate::output::writers::Writers;
use crate::types::SavedQuery;

use super::{explicit_sort_order, saved_query_from_url, UrlQueryOverrides};

/// Reset the named field of a saved query to its empty/unset state. The name
/// matches the long flag (kebab-case).
fn clear_query_field(q: &mut SavedQuery, field: &str) -> Result<()> {
    match field {
        "product" => q.product.clear(),
        "component" => q.component.clear(),
        "status" => q.status.clear(),
        "assignee" => q.assignee.clear(),
        "creator" => q.creator.clear(),
        "priority" => q.priority.clear(),
        "severity" => q.severity.clear(),
        "whiteboard" => q.whiteboard.clear(),
        "target-milestone" => q.target_milestone.clear(),
        "version" => q.version.clear(),
        "op-sys" => q.op_sys.clear(),
        "platform" => q.platform.clear(),
        "resolution" => q.resolution.clear(),
        "qa-contact" => q.qa_contact.clear(),
        "url" => q.url.clear(),
        "search" => q.quicksearch = None,
        "limit" => q.limit = None,
        "fields" => q.fields = None,
        "exclude-fields" => q.exclude_fields = None,
        "created-since" => q.creation_time = None,
        "changed-since" => q.last_change_time = None,
        "sort" | "order" => q.order = None,
        other => {
            return Err(BzrError::InputValidation(format!(
                "unknown --clear field '{other}'; see `bzr query update --help` for valid names"
            )))
        }
    }
    Ok(())
}

/// Merge a `query update` action's supplied flags into `q` in place: filter
/// flags replace lists, scalars replace values, `--clear` resets fields.
/// `creation_time`/`last_change_time` are the pre-validated canonical dates.
/// Returns `true` if any change was requested (so the caller can reject a
/// no-op call).
fn apply_query_updates(
    q: &mut SavedQuery,
    args: &crate::cli::QueryUpdateArgs,
    creation_time: Option<&str>,
    last_change_time: Option<&str>,
) -> Result<bool> {
    let crate::cli::QueryUpdateArgs {
        search,
        filters,
        actor_filters,
        limit,
        fields,
        exclude_fields,
        created_since,
        changed_since,
        clear,
        sort_args,
        ..
    } = args;
    let mut changed = false;
    changed |= filters.merge_saved_query_filters(q);
    changed |= actor_filters.merge_saved_query_filters(q);
    changed |= merge_set(&mut q.quicksearch, search.as_deref());
    changed |= merge_set(&mut q.fields, fields.as_deref());
    changed |= merge_set(&mut q.exclude_fields, exclude_fields.as_deref());
    if let Some(l) = limit {
        q.limit = Some(*l);
        changed = true;
    }
    if created_since.is_some() {
        q.creation_time = creation_time.map(ToOwned::to_owned);
        changed = true;
    }
    if changed_since.is_some() {
        q.last_change_time = last_change_time.map(ToOwned::to_owned);
        changed = true;
    }
    if sort_args.sort.is_some() {
        q.order = explicit_sort_order(sort_args);
        changed = true;
    }
    for field in clear {
        clear_query_field(q, field)?;
        changed = true;
    }
    Ok(changed)
}

pub(super) fn handle(
    args: &crate::cli::QueryUpdateArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let crate::cli::QueryUpdateArgs {
        name,
        from_url,
        limit,
        fields,
        exclude_fields,
        created_since,
        changed_since,
        sort_args,
        ..
    } = args;

    // Validate dates before acquiring the lock so a bad value exits cleanly.
    let creation_time =
        crate::validation::parse_optional_date(created_since.as_deref(), "--created-since")?;
    let last_change_time =
        crate::validation::parse_optional_date(changed_since.as_deref(), "--changed-since")?;
    let replacement = from_url
        .as_deref()
        .map(|url_str| {
            saved_query_from_url(
                url_str,
                UrlQueryOverrides {
                    limit: *limit,
                    fields: fields.as_deref(),
                    exclude_fields: exclude_fields.as_deref(),
                    creation_time: creation_time.as_deref(),
                    last_change_time: last_change_time.as_deref(),
                    sort_args,
                },
                ctx.config_path_override(),
            )
        })
        .transpose()?;

    Config::update_locked_at(ctx.config_path_override(), |config| {
        if let Some(query) = replacement {
            if !config.queries.contains_key(name.as_str()) {
                return Err(BzrError::config(format!("query '{name}' not found")));
            }
            if !query.has_filters() {
                return Err(BzrError::InputValidation(
                    "update would leave the query with no filters; a saved query must keep at \
                     least one filter set"
                        .into(),
                ));
            }
            config.queries.insert(name.clone(), query);
            return Ok(());
        }

        let Some(q) = config.queries.get_mut(name.as_str()) else {
            return Err(BzrError::config(format!("query '{name}' not found")));
        };
        let changed = apply_query_updates(
            q,
            args,
            creation_time.as_deref(),
            last_change_time.as_deref(),
        )?;
        if !changed {
            return Err(BzrError::InputValidation(
                "no changes specified: provide a filter/field flag or --clear <field>".into(),
            ));
        }
        if !q.has_filters() {
            return Err(BzrError::InputValidation(
                "update would leave the query with no filters; a saved query must keep at \
                 least one filter set"
                    .into(),
            ));
        }
        Ok(())
    })?;

    write_query_saved(name, "Updated", ctx.format(), w.out);
    Ok(())
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
