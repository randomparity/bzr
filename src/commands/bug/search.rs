use crate::cli::BugAction;
use crate::error::Result;
use crate::output::resources::bug::{canonical_field_list, write_bugs, ColumnSpec};
use crate::output::resources::query::write_query_saved;
use crate::output::writers::Writers;
use crate::types::{ApiMode, OutputFormat, Overrides, SavedQuery, SearchParams};

/// Default cap on bugs returned by a search when neither the URL nor `--limit`
/// specifies one. Keeps unbounded `bug search` invocations from pulling an
/// entire installation's bug list.
const DEFAULT_SEARCH_LIMIT: u32 = 50;

/// Determine the `save_as` name + query to persist after a successful URL-based
/// search. Returns None when --save-as wasn't passed; errors when --save-as=""
/// is passed but the URL has no `known_name`/`query_based_on` to fall back on.
fn resolve_save_info(
    save_as: Option<&String>,
    suggested_name: Option<String>,
    parsed_query: &SavedQuery,
) -> Result<Option<(String, SavedQuery)>> {
    let Some(raw_name) = save_as else {
        return Ok(None);
    };
    let name = if raw_name.is_empty() {
        suggested_name.ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "no name provided for --save-as and URL has no known_name; \
                 specify a name explicitly: --save-as <name>"
                    .into(),
            )
        })?
    } else {
        raw_name.clone()
    };
    Ok(Some((name, parsed_query.clone())))
}

/// Convert a parsed URL's query into `SearchParams`, applying CLI overrides
/// and a default limit of 50 when neither URL nor CLI specifies one.
fn build_params_from_url(
    parsed_query: SavedQuery,
    limit: Option<u32>,
    fields: Option<&str>,
    exclude_fields: Option<&str>,
) -> SearchParams {
    let mut params = parsed_query.into_search_params();
    if params.limit.is_none() && limit.is_none() {
        params.limit = Some(DEFAULT_SEARCH_LIMIT);
    }
    params.apply_overrides(Overrides {
        limit,
        fields,
        exclude_fields,
        ..Default::default()
    });
    params
}

/// Handles bug search — builds its own client (unlike other handlers) because
/// `--from-url` may resolve a different server from the URL hostname.
pub(super) async fn handle(
    action: &BugAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    let BugAction::Search {
        query,
        from_url,
        save_as,
        limit,
        fields,
        exclude_fields,
    } = action
    else {
        unreachable!()
    };

    let spec = ColumnSpec::new(fields.as_deref(), exclude_fields.as_deref());

    let (client, params, save_info) = if let Some(url_str) = from_url {
        let config = crate::config::Config::load()?;
        let parsed = crate::url_parser::parse_bugzilla_url(url_str, &config)?;
        let effective_server = server.or(parsed.query.server.as_deref());
        let client = crate::commands::shared::connect_and_configure(effective_server, api).await?;
        let save_info = resolve_save_info(save_as.as_ref(), parsed.suggested_name, &parsed.query)?;
        let canonical_fields = canonical_field_list(fields.as_deref());
        let canonical_exclude = canonical_field_list(exclude_fields.as_deref());
        let params = build_params_from_url(
            parsed.query,
            *limit,
            canonical_fields.as_deref(),
            canonical_exclude.as_deref(),
        );
        (client, params, save_info)
    } else {
        let query_str = query.as_deref().ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "either a search query or --from-url is required".into(),
            )
        })?;
        let client = crate::commands::shared::connect_and_configure(server, api).await?;
        let params = SearchParams {
            quicksearch: Some(query_str.to_string()),
            limit: Some(limit.unwrap_or(DEFAULT_SEARCH_LIMIT)),
            include_fields: canonical_field_list(fields.as_deref()),
            exclude_fields: canonical_field_list(exclude_fields.as_deref()),
            ..Default::default()
        };
        (client, params, None)
    };

    let bugs = client.search_bugs(&params).await?;
    write_bugs(&bugs, spec, format, w.out, w.err);

    if let Some((name, query)) = save_info {
        let mut config = crate::config::Config::load()?;
        let is_update = config.queries.contains_key(name.as_str());
        config.queries.insert(name.clone(), query);
        config.save()?;
        let verb = if is_update { "Updated" } else { "Saved" };
        write_query_saved(&name, verb, format, w.out);
    }

    Ok(())
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
