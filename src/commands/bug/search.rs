use crate::cli::{FieldArgs, SearchArgs};
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
    args: &SearchArgs,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    let SearchArgs {
        query,
        from_url,
        save_as,
        limit,
        count,
        field_args: FieldArgs {
            fields,
            exclude_fields,
        },
        sort_args,
        page_args: crate::cli::PageArgs { offset, paginate },
    } = args;

    super::ensure_no_paging_with_count(*count, *offset, *paginate)?;

    let spec = ColumnSpec::new(fields.as_deref(), exclude_fields.as_deref());

    let (client, mut params, save_info) = if let Some(url_str) = from_url {
        let config = crate::config::Config::load()?;
        let parsed = crate::url_parser::parse_bugzilla_url(url_str, &config)?;
        let effective_server = server.or(parsed.query.server.as_deref());
        let client = crate::commands::shared::connect_and_configure(effective_server, api).await?;
        let save_info = resolve_save_info(save_as.as_ref(), parsed.suggested_name, &parsed.query)?;
        let canonical_fields = canonical_field_list(fields.as_deref());
        let canonical_exclude = canonical_field_list(exclude_fields.as_deref());
        let mut params = build_params_from_url(
            parsed.query,
            *limit,
            canonical_fields.as_deref(),
            canonical_exclude.as_deref(),
        );
        // `--sort` overrides the URL's own ordering; otherwise the parsed URL
        // order (if any) is preserved verbatim.
        if sort_args.sort.is_some() {
            params.order = Some(crate::validation::build_order(
                sort_args.sort.as_deref(),
                sort_args.order,
            ));
        }
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
            order: Some(crate::validation::build_order(
                sort_args.sort.as_deref(),
                sort_args.order,
            )),
            ..Default::default()
        };
        (client, params, None)
    };
    crate::commands::paging::resolve_offset(&mut params, *offset);

    if *count {
        let bugs = client
            .search_bugs(&super::count_search_params(params))
            .await?;
        crate::output::result_types::write_count(bugs.len(), format, w.out);
    } else {
        let page = crate::commands::paging::fetch_page(&client, &params, *paginate).await?;
        write_bugs(&page.bugs, spec, format, w.out, w.err);
        crate::commands::paging::write_truncation_note(&page, params.limit, *offset, format, w);
    }

    if let Some((name, query)) = save_info {
        let mut is_update = false;
        crate::config::Config::update_locked(|config| {
            is_update = config.queries.contains_key(name.as_str());
            config.queries.insert(name.clone(), query);
            Ok(())
        })?;
        let verb = if is_update { "Updated" } else { "Saved" };
        write_query_saved(&name, verb, format, w.out);
    }

    Ok(())
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
