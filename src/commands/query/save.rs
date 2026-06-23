use crate::commands::runtime::context::CommandContext;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::resources::query::write_query_saved;
use crate::output::writers::Writers;
use crate::types::bug::SavedQuery;

use super::{explicit_sort_order, saved_query_from_url, UrlQueryOverrides};

pub(super) fn handle(
    args: &crate::cli::SaveArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let crate::cli::SaveArgs {
        name,
        from_url,
        search,
        filters,
        actor_filters,
        limit,
        fields,
        exclude_fields,
        created_since,
        changed_since,
        sort_args,
    } = args;

    let creation_time =
        crate::validation::parse_optional_date(created_since.as_deref(), "--created-since")?;
    let last_change_time =
        crate::validation::parse_optional_date(changed_since.as_deref(), "--changed-since")?;

    let query = if let Some(url_str) = from_url {
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
        )?
    } else {
        let mut query = SavedQuery {
            quicksearch: search.clone(),
            limit: *limit,
            fields: fields.clone(),
            exclude_fields: exclude_fields.clone(),
            creation_time,
            last_change_time,
            order: explicit_sort_order(sort_args),
            ..SavedQuery::default()
        };
        filters.write_saved_query_filters(&mut query);
        actor_filters.write_saved_query_filters(&mut query);
        query
    };

    if !query.has_filters() {
        return Err(BzrError::InputValidation(
            "query must have at least one filter set".into(),
        ));
    }

    let mut is_update = false;
    Config::update_locked_at(ctx.config_path_override(), |config| {
        is_update = config.queries.contains_key(name.as_str());
        config.queries.insert(name.clone(), query);
        Ok(())
    })?;

    let verb = if is_update { "Updated" } else { "Saved" };
    write_query_saved(name, verb, ctx.format(), w.out);
    Ok(())
}
