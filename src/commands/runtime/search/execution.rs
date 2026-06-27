use crate::client::BugzillaClient;
use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::search::fields::{
    validate_json_field_selection, validate_table_columns, warn_unknown_fields, ColumnSpec,
};
use crate::commands::runtime::search::policy::count_search_params;
use crate::error::Result;
use crate::output::resources::bug::write_bugs;
use crate::output::resources::query::write_query_saved;
use crate::output::writers::Writers;
use crate::types::bug::SearchParams;
use crate::types::output::OutputFormat;
use crate::types::query::SavedQuery;

#[derive(Clone, Copy)]
pub(crate) enum FieldPreflight {
    AlreadyDone,
    Validate,
}

pub(crate) struct SearchColumns {
    include: Option<String>,
    exclude: Option<String>,
}

impl SearchColumns {
    pub(crate) fn from_params(params: &SearchParams) -> Self {
        Self {
            include: params.include_fields.clone(),
            exclude: params.exclude_fields.clone(),
        }
    }

    fn spec(&self) -> ColumnSpec<'_> {
        ColumnSpec {
            include: self.include.as_deref(),
            exclude: self.exclude.as_deref(),
        }
    }
}

pub(crate) struct SearchSaveAction {
    pub(crate) name: String,
    pub(crate) query: SavedQuery,
}

pub(crate) struct SearchExecutionPlan<'a> {
    pub(crate) client: &'a BugzillaClient,
    pub(crate) params: SearchParams,
    pub(crate) columns: SearchColumns,
    pub(crate) count: bool,
    pub(crate) paginate: bool,
    pub(crate) offset: Option<u32>,
    pub(crate) field_preflight: FieldPreflight,
    pub(crate) save: Option<SearchSaveAction>,
}

pub(crate) async fn execute(
    plan: SearchExecutionPlan<'_>,
    ctx: &CommandContext,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    if plan.count {
        let bugs = plan
            .client
            .search_bugs(&count_search_params(plan.params))
            .await?;
        crate::output::result_types::write_count(bugs.len(), format, w.out);
        return persist_saved_query(plan.save, ctx, format, w.out);
    }

    let spec = plan.columns.spec();
    validate_fields(plan.field_preflight, spec, format, w.err)?;

    let page = crate::commands::runtime::paging::fetch_page(
        plan.client,
        &plan.params,
        plan.paginate,
        ctx.progress(),
        w,
    )
    .await?;
    write_bugs(&page.bugs, spec, format, w.out, w.err);
    crate::commands::runtime::paging::write_truncation_note(
        &page,
        plan.params.limit,
        plan.offset,
        format,
        w,
    );

    // Persist the saved query (a fallible local config write) before the
    // terminal `done`: a save failure must surface as `error` with no preceding
    // `done`, preserving the "done only on full success" guarantee.
    persist_saved_query(plan.save, ctx, format, w.out)?;
    if plan.paginate {
        crate::output::progress::done_event(ctx.progress(), w.err, page.bugs.len());
    }
    Ok(())
}

fn validate_fields(
    field_preflight: FieldPreflight,
    spec: ColumnSpec<'_>,
    format: OutputFormat,
    err: &mut dyn std::io::Write,
) -> Result<()> {
    if let FieldPreflight::AlreadyDone = field_preflight {
        return Ok(());
    }
    match format {
        OutputFormat::Table => validate_table_columns(spec)?,
        OutputFormat::Json | OutputFormat::Ndjson => {
            validate_json_field_selection(spec)?;
            warn_unknown_fields(spec, err);
        }
    }
    Ok(())
}

fn persist_saved_query(
    save: Option<SearchSaveAction>,
    ctx: &CommandContext,
    format: OutputFormat,
    out: &mut dyn std::io::Write,
) -> Result<()> {
    let Some(SearchSaveAction { name, query }) = save else {
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
