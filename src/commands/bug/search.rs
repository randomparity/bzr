use crate::cli::SearchArgs;
use crate::client::BugzillaClient;
use crate::commands::runtime::invocation::CommandContext;
use crate::commands::runtime::search::execution::{
    FieldPreflight, SearchColumns, SearchExecutionPlan, SearchSaveAction,
};
use crate::commands::runtime::search::fields::canonical_field_list;
use crate::commands::runtime::search::policy::ensure_no_paging_with_count;
use crate::error::Result;
use crate::output::writers::Writers;
use crate::types::bug::{Overrides, SearchParams};
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
            crate::error::BzrError::input(
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
        if args.query.is_none() && args.saved_search.is_none() {
            return Err(crate::error::BzrError::input(
                "a search query, --saved-search, or --from-url is required".into(),
            ));
        }
        let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
        // A saved search is a Red Hat extension: refuse before dispatch rather
        // than let a stock server silently return an unfiltered result
        // (ADR-0052). Clap guarantees at most one query source is set here.
        if let Some(name) = args.saved_search.as_deref() {
            crate::commands::runtime::shared::require_server_capability(
                ctx,
                &client,
                crate::commands::runtime::shared::RED_HAT_EXTENSION,
                &format!("saved search '{name}'"),
            )
            .await?;
        }
        let params = SearchParams {
            quicksearch: args.query.clone(),
            saved_search: args.saved_search.clone(),
            sharer_id: args.sharer,
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
    let parsed = crate::commands::runtime::input::url_parser::parse_bugzilla_url(
        url_str,
        &config,
        ctx.inline_server().map(|server| server.url.as_str()),
    )?;
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

    let (client, mut params, save_info) = resolve_client_and_params(args, ctx).await?;
    crate::commands::runtime::search::paging::resolve_page_window(&mut params, offset);
    let columns = SearchColumns::from_params(&params);
    let field_preflight = if args.from_url.is_some() {
        FieldPreflight::Validate
    } else {
        FieldPreflight::AlreadyDone
    };
    let save = save_info.map(|(name, query)| SearchSaveAction { name, query });
    crate::commands::runtime::search::execution::execute(
        SearchExecutionPlan {
            client: &client,
            params,
            columns,
            count: args.count,
            paginate: args.page_args.paginate,
            offset,
            field_preflight,
            save,
        },
        ctx,
        format,
        w,
    )
    .await
}

#[cfg(test)]
#[path = "search_tests.rs"]
mod tests;
