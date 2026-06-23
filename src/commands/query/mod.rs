//! Saved query management commands.
//!
//! Query operations (save/list/show/delete) are pure local file I/O.
//! Only `run` requires a network client.

use crate::cli::QueryAction;
use crate::commands::runtime::context::CommandContext;
use crate::config::Config;
use crate::error::Result;
use crate::output::writers::Writers;
use crate::types::query::SavedQuery;

mod delete;
mod list;
mod run;
mod save;
mod show;
mod update;

pub async fn execute(
    action: &QueryAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        QueryAction::Save(args) => save::handle(args, ctx, w),
        QueryAction::List => list::handle(ctx, w),
        QueryAction::Show(args) => show::handle(args, ctx, w),
        QueryAction::Update(args) => update::handle(args, ctx, w),
        QueryAction::Delete(args) => delete::handle(args, ctx, w),
        QueryAction::Run(args) => run::handle(args, ctx, w).await,
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

#[derive(Clone, Copy)]
struct UrlQueryOverrides<'a> {
    limit: Option<u32>,
    fields: Option<&'a str>,
    exclude_fields: Option<&'a str>,
    creation_time: Option<&'a str>,
    last_change_time: Option<&'a str>,
    sort_args: &'a crate::cli::SortArgs,
}

fn saved_query_from_url(
    url_str: &str,
    overrides: UrlQueryOverrides<'_>,
    config_path_override: Option<&std::path::Path>,
) -> Result<SavedQuery> {
    let config = Config::load_at(config_path_override)?;
    let parsed = crate::commands::runtime::url_parser::parse_bugzilla_url(url_str, &config)?;
    let mut query = parsed.query;
    query.limit = overrides.limit.or(query.limit);
    query.fields = overrides.fields.map(ToOwned::to_owned).or(query.fields);
    query.exclude_fields = overrides
        .exclude_fields
        .map(ToOwned::to_owned)
        .or(query.exclude_fields);
    query.creation_time = overrides
        .creation_time
        .map(ToOwned::to_owned)
        .or(query.creation_time);
    query.last_change_time = overrides
        .last_change_time
        .map(ToOwned::to_owned)
        .or(query.last_change_time);
    query.order = explicit_sort_order(overrides.sort_args);
    Ok(query)
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
