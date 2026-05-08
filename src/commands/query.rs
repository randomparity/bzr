//! Saved query management commands.
//!
//! Query operations (save/list/show/delete) are pure local file I/O.
//! Only `run` requires a network client.

use crate::cli::QueryAction;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::{self, Writers};
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
        QueryAction::Save { .. } => handle_save(action, format, w),
        QueryAction::List => handle_list(format, w),
        QueryAction::Show { .. } => handle_show(action, format, w),
        QueryAction::Delete { .. } => handle_delete(action, format, w),
        QueryAction::Run { .. } => handle_run(action, server, format, api, w).await,
    }
}

fn handle_save(action: &QueryAction, format: OutputFormat, w: &mut Writers<'_>) -> Result<()> {
    let QueryAction::Save {
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
    } = action
    else {
        unreachable!()
    };

    let creation_time =
        crate::validation::parse_optional_date(created_since.as_deref(), "--created-since")?;
    let last_change_time =
        crate::validation::parse_optional_date(changed_since.as_deref(), "--changed-since")?;

    let (query, preloaded_config) = if let Some(url_str) = from_url {
        let config = Config::load()?;
        let parsed = crate::url_parser::parse_bugzilla_url(url_str, &config)?;
        let mut query = parsed.query;
        if let Some(limit) = limit {
            query.limit = Some(*limit);
        }
        if let Some(f) = fields {
            query.fields = Some(f.clone());
        }
        if let Some(ef) = exclude_fields {
            query.exclude_fields = Some(ef.clone());
        }
        if creation_time.is_some() {
            query.creation_time = creation_time;
        }
        if last_change_time.is_some() {
            query.last_change_time = last_change_time;
        }
        (query, Some(config))
    } else {
        let kind = if search.is_some() {
            QueryKind::Search
        } else {
            QueryKind::List
        };
        let query = SavedQuery {
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
            ..SavedQuery::default()
        };
        (query, None)
    };

    if !query.has_filters() {
        return Err(BzrError::InputValidation(
            "query must have at least one filter set".into(),
        ));
    }

    let mut config = match preloaded_config {
        Some(c) => c,
        None => Config::load()?,
    };
    let is_update = config.queries.contains_key(name.as_str());
    config.queries.insert(name.clone(), query);
    config.save()?;

    let verb = if is_update { "Updated" } else { "Saved" };
    output::write_query_saved(name, verb, format, w.out);
    Ok(())
}

fn handle_list(format: OutputFormat, w: &mut Writers<'_>) -> Result<()> {
    let config = Config::load()?;
    output::write_query_list(&config.queries, format, w.out);
    Ok(())
}

fn handle_show(action: &QueryAction, format: OutputFormat, w: &mut Writers<'_>) -> Result<()> {
    let QueryAction::Show { name } = action else {
        unreachable!()
    };
    let config = Config::load()?;
    let query = config
        .queries
        .get(name.as_str())
        .ok_or_else(|| BzrError::config(format!("query '{name}' not found")))?;
    output::write_query_detail(name, query, format, w.out);
    Ok(())
}

fn handle_delete(action: &QueryAction, format: OutputFormat, w: &mut Writers<'_>) -> Result<()> {
    let QueryAction::Delete { name } = action else {
        unreachable!()
    };
    let mut config = Config::load()?;
    if config.queries.remove(name.as_str()).is_none() {
        return Err(BzrError::config(format!("query '{name}' not found")));
    }
    config.save()?;

    output::write_query_saved(name, "Deleted", format, w.out);
    Ok(())
}

async fn handle_run(
    action: &QueryAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<crate::types::ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    let QueryAction::Run {
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
    } = action
    else {
        unreachable!()
    };

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

    let client = super::shared::connect_and_configure(effective_server, api).await?;
    let bugs = client.search_bugs(&params).await?;
    output::write_bugs(&bugs, format, w.out);
    Ok(())
}

#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;
