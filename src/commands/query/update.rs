use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::shared::merge_set;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::resources::query::write_query_saved;
use crate::output::writers::Writers;
use crate::types::query::SavedQuery;

use super::{explicit_sort_order, saved_query_from_url, UrlQueryOverrides};

/// Reset the named field of a saved query to its empty/unset state. The name
/// matches the long flag (kebab-case).
fn clear_query_field(saved_query: &mut SavedQuery, field: &str) -> Result<()> {
    match field {
        "product" => saved_query.product.clear(),
        "component" => saved_query.component.clear(),
        "status" => saved_query.status.clear(),
        "assignee" => saved_query.assignee.clear(),
        "creator" => saved_query.creator.clear(),
        "priority" => saved_query.priority.clear(),
        "severity" => saved_query.severity.clear(),
        "whiteboard" => saved_query.whiteboard.clear(),
        "target-milestone" => saved_query.target_milestone.clear(),
        "version" => saved_query.version.clear(),
        "op-sys" => saved_query.op_sys.clear(),
        "platform" => saved_query.platform.clear(),
        "resolution" => saved_query.resolution.clear(),
        "qa-contact" => saved_query.qa_contact.clear(),
        "url" => saved_query.url.clear(),
        "search" => saved_query.quicksearch = None,
        "limit" => saved_query.limit = None,
        "fields" => saved_query.fields = None,
        "exclude-fields" => saved_query.exclude_fields = None,
        "created-since" => saved_query.creation_time = None,
        "changed-since" => saved_query.last_change_time = None,
        "sort" | "order" => saved_query.order = None,
        other => {
            return Err(BzrError::InputValidation(format!(
                "unknown --clear field '{other}'; see `bzr query update --help` for valid names"
            )))
        }
    }
    Ok(())
}

/// Merge a `query update` action's supplied flags into `saved_query` in place:
/// filter flags replace lists, scalars replace values, `--clear` resets fields.
/// `creation_time`/`last_change_time` are the pre-validated canonical dates.
/// Returns `true` if any change was requested (so the caller can reject a
/// no-op call).
fn apply_query_updates(
    saved_query: &mut SavedQuery,
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
    changed |= filters.merge_saved_query_filters(saved_query);
    changed |= actor_filters.merge_saved_query_filters(saved_query);
    changed |= merge_set(&mut saved_query.quicksearch, search.as_deref());
    changed |= merge_set(&mut saved_query.fields, fields.as_deref());
    changed |= merge_set(&mut saved_query.exclude_fields, exclude_fields.as_deref());
    if let Some(l) = limit {
        saved_query.limit = Some(*l);
        changed = true;
    }
    if created_since.is_some() {
        saved_query.creation_time = creation_time.map(ToOwned::to_owned);
        changed = true;
    }
    if changed_since.is_some() {
        saved_query.last_change_time = last_change_time.map(ToOwned::to_owned);
        changed = true;
    }
    if sort_args.sort.is_some() {
        saved_query.order = explicit_sort_order(sort_args);
        changed = true;
    }
    for field in clear {
        clear_query_field(saved_query, field)?;
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

        let Some(saved_query) = config.queries.get_mut(name.as_str()) else {
            return Err(BzrError::config(format!("query '{name}' not found")));
        };
        let changed = apply_query_updates(
            saved_query,
            args,
            creation_time.as_deref(),
            last_change_time.as_deref(),
        )?;
        if !changed {
            return Err(BzrError::InputValidation(
                "no changes specified: provide a filter/field flag or --clear <field>".into(),
            ));
        }
        if !saved_query.has_filters() {
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
