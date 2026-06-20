//! Saved query management commands.
//!
//! Query operations (save/list/show/delete) are pure local file I/O.
//! Only `run` requires a network client.

use crate::cli::QueryAction;
use crate::commands::runtime::shared::{merge_set, merge_vec};
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::resources::bug::{
    canonical_field_list, validate_json_field_selection, validate_table_columns,
    warn_unknown_fields, write_bugs, ColumnSpec,
};
use crate::output::resources::query::{write_query_detail, write_query_list, write_query_saved};
use crate::output::writers::Writers;
use crate::types::{OutputFormat, QueryKind, SavedQuery};

/// Translate clap's empty-Vec sentinel into `None` so
/// `Overrides`'s `Option<&[String]>` semantics line up: an absent
/// flag keeps the saved value, a non-empty flag replaces it.
fn slice_override(v: &[String]) -> Option<&[String]> {
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

pub async fn execute(
    action: &QueryAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<crate::types::ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        QueryAction::Save(args) => handle_save(args, format, w),
        QueryAction::List => handle_list(format, w),
        QueryAction::Show(args) => handle_show(args, format, w),
        QueryAction::Update(args) => handle_update(args, format, w),
        QueryAction::Delete(args) => handle_delete(args, format, w),
        QueryAction::Run(args) => handle_run(args, server, format, api, w).await,
    }
}

/// The `order` clause to persist for a saved query: `Some` only when `--sort`
/// is given, so `query run` applies its own stable default otherwise.
fn explicit_sort_order(sort_args: &crate::cli::SortArgs) -> Option<String> {
    sort_args
        .sort
        .as_ref()
        .map(|_| crate::validation::build_order(sort_args.sort.as_deref(), sort_args.order))
}

fn handle_save(
    args: &crate::cli::query::SaveArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let crate::cli::query::SaveArgs {
        name,
        from_url,
        search,
        product,
        component,
        status,
        assignee,
        creator,
        priority,
        severity,
        limit,
        fields,
        exclude_fields,
        created_since,
        changed_since,
        whiteboard,
        target_milestone,
        version,
        op_sys,
        platform,
        resolution,
        qa_contact,
        url,
        sort_args,
    } = args;

    let creation_time =
        crate::validation::parse_optional_date(created_since.as_deref(), "--created-since")?;
    let last_change_time =
        crate::validation::parse_optional_date(changed_since.as_deref(), "--changed-since")?;

    let query = if let Some(url_str) = from_url {
        let config = Config::load()?;
        let parsed = crate::url_parser::parse_bugzilla_url(url_str, &config)?;
        let mut query = parsed.query;
        // A flag value overrides the URL-parsed value; otherwise the parsed
        // value is kept.
        query.limit = (*limit).or(query.limit);
        query.fields = fields.clone().or(query.fields);
        query.exclude_fields = exclude_fields.clone().or(query.exclude_fields);
        query.creation_time = creation_time.or(query.creation_time);
        query.last_change_time = last_change_time.or(query.last_change_time);
        query.order = explicit_sort_order(sort_args);
        query
    } else {
        let kind = if search.is_some() {
            QueryKind::Search
        } else {
            QueryKind::List
        };
        SavedQuery {
            kind,
            product: product.clone(),
            component: component.clone(),
            status: status.clone(),
            assignee: assignee.clone(),
            creator: creator.clone(),
            priority: priority.clone(),
            severity: severity.clone(),
            quicksearch: search.clone(),
            limit: *limit,
            fields: fields.clone(),
            exclude_fields: exclude_fields.clone(),
            creation_time,
            last_change_time,
            whiteboard: whiteboard.clone(),
            target_milestone: target_milestone.clone(),
            version: version.clone(),
            op_sys: op_sys.clone(),
            platform: platform.clone(),
            resolution: resolution.clone(),
            qa_contact: qa_contact.clone(),
            url: url.clone(),
            order: explicit_sort_order(sort_args),
            ..SavedQuery::default()
        }
    };

    if !query.has_filters() {
        return Err(BzrError::InputValidation(
            "query must have at least one filter set".into(),
        ));
    }

    let mut is_update = false;
    Config::update_locked(|config| {
        is_update = config.queries.contains_key(name.as_str());
        config.queries.insert(name.clone(), query);
        Ok(())
    })?;

    let verb = if is_update { "Updated" } else { "Saved" };
    write_query_saved(name, verb, format, w.out);
    Ok(())
}

fn handle_list(format: OutputFormat, w: &mut Writers<'_>) -> Result<()> {
    let config = Config::load()?;
    write_query_list(&config.queries, format, w.out);
    Ok(())
}

fn handle_show(
    args: &crate::cli::query::ShowArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let name = &args.name;
    let config = Config::load()?;
    let query = config
        .queries
        .get(name.as_str())
        .ok_or_else(|| BzrError::config(format!("query '{name}' not found")))?;
    write_query_detail(name, query, format, w.out);
    Ok(())
}

fn handle_delete(
    args: &crate::cli::query::DeleteArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let name = &args.name;
    Config::update_locked(|config| {
        if config.queries.remove(name.as_str()).is_none() {
            return Err(BzrError::config(format!("query '{name}' not found")));
        }
        Ok(())
    })?;

    write_query_saved(name, "Deleted", format, w.out);
    Ok(())
}

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
    args: &crate::cli::query::UpdateArgs,
    creation_time: Option<&str>,
    last_change_time: Option<&str>,
) -> Result<bool> {
    let crate::cli::query::UpdateArgs {
        search,
        product,
        component,
        status,
        assignee,
        creator,
        priority,
        severity,
        limit,
        fields,
        exclude_fields,
        created_since,
        changed_since,
        whiteboard,
        target_milestone,
        version,
        op_sys,
        platform,
        resolution,
        qa_contact,
        url,
        clear,
        sort_args,
        ..
    } = args;
    let mut changed = false;
    changed |= merge_vec(&mut q.product, product);
    changed |= merge_vec(&mut q.component, component);
    changed |= merge_vec(&mut q.status, status);
    changed |= merge_vec(&mut q.assignee, assignee);
    changed |= merge_vec(&mut q.creator, creator);
    changed |= merge_vec(&mut q.priority, priority);
    changed |= merge_vec(&mut q.severity, severity);
    changed |= merge_vec(&mut q.whiteboard, whiteboard);
    changed |= merge_vec(&mut q.target_milestone, target_milestone);
    changed |= merge_vec(&mut q.version, version);
    changed |= merge_vec(&mut q.op_sys, op_sys);
    changed |= merge_vec(&mut q.platform, platform);
    changed |= merge_vec(&mut q.resolution, resolution);
    changed |= merge_vec(&mut q.qa_contact, qa_contact);
    changed |= merge_vec(&mut q.url, url);
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

fn handle_update(
    args: &crate::cli::query::UpdateArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let crate::cli::query::UpdateArgs {
        name,
        created_since,
        changed_since,
        ..
    } = args;

    // Validate dates before acquiring the lock so a bad value exits cleanly.
    let creation_time =
        crate::validation::parse_optional_date(created_since.as_deref(), "--created-since")?;
    let last_change_time =
        crate::validation::parse_optional_date(changed_since.as_deref(), "--changed-since")?;

    Config::update_locked(|config| {
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

    write_query_saved(name, "Updated", format, w.out);
    Ok(())
}

async fn handle_run(
    args: &crate::cli::query::RunArgs,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<crate::types::ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    let crate::cli::query::RunArgs {
        name,
        limit,
        fields,
        exclude_fields,
        server: server_override,
        created_since,
        changed_since,
        whiteboard,
        target_milestone,
        version,
        op_sys,
        platform,
        resolution,
        qa_contact,
        url,
        sort_args,
        page_args: crate::cli::PageArgs { offset, paginate },
    } = args;

    let creation_time_override =
        crate::validation::parse_optional_date(created_since.as_deref(), "--created-since")?;
    let last_change_time_override =
        crate::validation::parse_optional_date(changed_since.as_deref(), "--changed-since")?;

    let config = Config::load()?;
    let saved = config
        .queries
        .get(name.as_str())
        .ok_or_else(|| BzrError::config(format!("query '{name}' not found")))?;

    let effective_server = server
        .or(server_override.as_deref())
        .or(saved.server.as_deref());

    let mut params = saved.to_search_params();
    params.apply_overrides(crate::types::Overrides {
        limit: *limit,
        fields: fields.as_deref(),
        exclude_fields: exclude_fields.as_deref(),
        creation_time: creation_time_override.as_deref(),
        last_change_time: last_change_time_override.as_deref(),
        whiteboard: slice_override(whiteboard),
        target_milestone: slice_override(target_milestone),
        version: slice_override(version),
        op_sys: slice_override(op_sys),
        platform: slice_override(platform),
        resolution: slice_override(resolution),
        qa_contact: slice_override(qa_contact),
        url: slice_override(url),
    });
    params.include_fields = canonical_field_list(params.include_fields.as_deref());
    params.exclude_fields = canonical_field_list(params.exclude_fields.as_deref());
    // Fold any saved-query/URL `offset` into the struct field and let `--offset`
    // override, so a saved-from-URL query never sends two `offset` params.
    crate::commands::runtime::paging::resolve_offset(&mut params, *offset);
    // Result ordering: an explicit `--sort` overrides the saved order; absent
    // both, default to a stable `bug_id` so runs are deterministic — unless the
    // saved query (e.g. from a URL) already carries an `order` raw param.
    if sort_args.sort.is_some() {
        params.order = Some(crate::validation::build_order(
            sort_args.sort.as_deref(),
            sort_args.order,
        ));
    } else if params.order.is_none() && !params.raw_params.iter().any(|(k, _)| k == "order") {
        params.order = Some(crate::validation::build_order(None, sort_args.order));
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

    let client = super::runtime::shared::connect_and_configure(effective_server, api).await?;
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

#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;
