use crate::commands::bug::search_support::fields::{
    canonical_field_list, validate_json_field_selection, validate_table_columns,
    warn_unknown_fields, ColumnSpec,
};
use crate::commands::bug::search_support::policy::{
    count_search_params, ensure_no_paging_with_count,
};
use crate::commands::runtime::context::CommandContext;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::resources::bug::write_bugs;
use crate::output::writers::Writers;
use crate::types::common::OutputFormat;

pub(super) async fn handle(
    args: &crate::cli::RunArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    let crate::cli::RunArgs {
        name,
        limit,
        count,
        fields,
        exclude_fields,
        server: server_override,
        created_since,
        changed_since,
        filters,
        url,
        sort_args,
        page_args: crate::cli::PageArgs { offset, paginate },
    } = args;

    ensure_no_paging_with_count(*count, *offset, *paginate)?;

    let creation_time_override =
        crate::validation::parse_optional_date(created_since.as_deref(), "--created-since")?;
    let last_change_time_override =
        crate::validation::parse_optional_date(changed_since.as_deref(), "--changed-since")?;

    let config = Config::load_at(ctx.config_path_override())?;
    let saved = config
        .queries
        .get(name.as_str())
        .ok_or_else(|| BzrError::config(format!("query '{name}' not found")))?;

    let effective_server = ctx
        .server()
        .or(server_override.as_deref())
        .or(saved.server.as_deref());
    let run_ctx = ctx.with_server(effective_server);

    let mut params = saved.to_search_params();
    let mut overrides = filters.overrides(url);
    overrides.limit = *limit;
    overrides.fields = fields.as_deref();
    overrides.exclude_fields = exclude_fields.as_deref();
    overrides.creation_time = creation_time_override.as_deref();
    overrides.last_change_time = last_change_time_override.as_deref();
    params.apply_overrides(overrides);
    params.include_fields = canonical_field_list(params.include_fields.as_deref());
    params.exclude_fields = canonical_field_list(params.exclude_fields.as_deref());
    // Fold any saved-query/URL `offset` into the struct field and let `--offset`
    // override, so a saved-from-URL query never sends two `offset` params.
    crate::commands::runtime::paging::resolve_offset(&mut params, *offset);
    // Result ordering: an explicit `--sort` overrides the saved order; absent
    // both, default to a stable `bug_id` so runs are deterministic, unless the
    // saved query (e.g. from a URL) already carries an `order` raw param.
    if sort_args.sort.is_some() {
        params.order = Some(crate::validation::build_order(
            sort_args.sort.as_deref(),
            sort_args.order,
        ));
    } else if params.order.is_none() && !params.raw_params.iter().any(|(k, _)| k == "order") {
        params.order = Some(crate::validation::build_order(None, sort_args.order));
    }

    if *count {
        let client = crate::commands::runtime::shared::connect_and_configure(&run_ctx).await?;
        let bugs = client.search_bugs(&count_search_params(params)).await?;
        crate::output::result_types::write_count(bugs.len(), format, w.out);
        return Ok(());
    }

    // Source columns from the resolved params (saved-query fields + CLI
    // overrides), not the raw flags, so a stored field selection is honored.
    let spec = ColumnSpec {
        include: params.include_fields.as_deref(),
        exclude: params.exclude_fields.as_deref(),
    };
    match format {
        OutputFormat::Table => validate_table_columns(spec)?,
        OutputFormat::Json | OutputFormat::Ndjson => {
            validate_json_field_selection(spec)?;
            warn_unknown_fields(spec, w.err);
        }
    }

    let client = crate::commands::runtime::shared::connect_and_configure(&run_ctx).await?;
    let page = crate::commands::runtime::paging::fetch_page(&client, &params, *paginate).await?;
    write_bugs(&page.bugs, spec, format, w.out, w.err);
    crate::commands::runtime::paging::write_truncation_note(
        &page,
        params.limit,
        *offset,
        format,
        w,
    );
    Ok(())
}
