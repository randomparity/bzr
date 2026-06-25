use std::io::Write;

use crate::cli::SearchArgs;
use crate::client::BugzillaClient;
use crate::commands::bug::search_support::fields::{canonical_field_list, ColumnSpec};
use crate::commands::bug::search_support::policy::{
    count_search_params, ensure_no_paging_with_count,
};
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::bug::write_bugs;
use crate::output::resources::query::write_query_saved;
use crate::output::writers::Writers;
use crate::types::bug::{Overrides, SearchParams};
use crate::types::output::OutputFormat;
use crate::types::query::SavedQuery;

/// The client plus the query to run, and any `--save-as` query to persist
/// afterwards. Produced by [`resolve_client_and_params`] from either the
/// `--from-url` or the quicksearch path.
type SearchPlan = (BugzillaClient, SearchParams, Option<(String, SavedQuery)>);

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

/// Resolve the search client and query from `--from-url` (which may target a
/// different server, parsed from the URL host) or from a quicksearch string.
async fn resolve_client_and_params(args: &SearchArgs, ctx: &CommandContext) -> Result<SearchPlan> {
    let fields = args.field_args.fields.as_deref();
    let exclude_fields = args.field_args.exclude_fields.as_deref();

    let Some(url_str) = args.from_url.as_deref() else {
        let query_str = args.query.as_deref().ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "either a search query or --from-url is required".into(),
            )
        })?;
        let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
        let params = SearchParams {
            quicksearch: Some(query_str.to_string()),
            limit: Some(args.limit.unwrap_or(DEFAULT_SEARCH_LIMIT)),
            include_fields: canonical_field_list(fields),
            exclude_fields: canonical_field_list(exclude_fields),
            order: Some(crate::validation::build_order(
                args.sort_args.sort.as_deref(),
                args.sort_args.order,
            )),
            ..Default::default()
        };
        return Ok((client, params, None));
    };

    let config = crate::config::Config::load_at(ctx.config_path_override())?;
    let parsed = crate::commands::runtime::url_parser::parse_bugzilla_url(url_str, &config)?;
    let effective_server = ctx.server().or(parsed.query.server.as_deref());
    let url_ctx = ctx.with_server(effective_server);
    let client = crate::commands::runtime::shared::connect_and_configure(&url_ctx).await?;
    let save_info = resolve_save_info(args.save_as.as_ref(), parsed.suggested_name, &parsed.query)?;
    let mut params = build_params_from_url(
        parsed.query,
        args.limit,
        canonical_field_list(fields).as_deref(),
        canonical_field_list(exclude_fields).as_deref(),
    );
    // `--sort` overrides the URL's own ordering; otherwise the parsed URL
    // order (if any) is preserved verbatim.
    if args.sort_args.sort.is_some() {
        params.order = Some(crate::validation::build_order(
            args.sort_args.sort.as_deref(),
            args.sort_args.order,
        ));
    }
    Ok((client, params, save_info))
}

/// Persist the `--save-as` query (insert or update) and report the result.
/// No-op when the search had no `--save-as`.
fn persist_saved_query(
    save_info: Option<(String, SavedQuery)>,
    ctx: &CommandContext,
    format: OutputFormat,
    out: &mut dyn Write,
) -> Result<()> {
    let Some((name, query)) = save_info else {
        return Ok(());
    };
    let mut is_update = false;
    crate::config::Config::update_locked_at(ctx.config_path_override(), |config| {
        is_update = config.queries.contains_key(name.as_str());
        config.queries.insert(name.clone(), query);
        Ok(())
    })?;
    let verb = if is_update { "Updated" } else { "Saved" };
    write_query_saved(&name, verb, format, out);
    Ok(())
}

/// Handles bug search — builds its own client (unlike other handlers) because
/// `--from-url` may resolve a different server from the URL hostname.
pub(super) async fn handle(
    args: &SearchArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    let offset = args.page_args.offset;
    ensure_no_paging_with_count(args.count, offset, args.page_args.paginate)?;

    let spec = ColumnSpec::new(
        args.field_args.fields.as_deref(),
        args.field_args.exclude_fields.as_deref(),
    );

    let (client, mut params, save_info) = resolve_client_and_params(args, ctx).await?;
    crate::commands::runtime::paging::resolve_offset(&mut params, offset);

    if args.count {
        let bugs = client.search_bugs(&count_search_params(params)).await?;
        crate::output::result_types::write_count(bugs.len(), format, w.out);
    } else {
        let page =
            crate::commands::runtime::paging::fetch_page(&client, &params, args.page_args.paginate)
                .await?;
        write_bugs(&page.bugs, spec, format, w.out, w.err);
        crate::commands::runtime::paging::write_truncation_note(
            &page,
            params.limit,
            offset,
            format,
            w,
        );
    }

    persist_saved_query(save_info, ctx, format, w.out)
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
