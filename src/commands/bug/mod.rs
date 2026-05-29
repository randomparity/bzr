//! Bug subcommand handlers, split per-action.

use crate::cli::BugAction;
use crate::error::Result;
use crate::output::resources::bug::{
    validate_json_field_selection, validate_table_columns, warn_unknown_fields, ColumnSpec,
};
use crate::output::writers::Writers;
use crate::types::{ApiMode, OutputFormat};

mod clone;
mod create;
mod history;
mod list;
mod my;
mod search;
mod update;
mod view;

/// The `--fields` / `--exclude-fields` column spec for the four field-bearing
/// bug actions, or `None` for actions that take no field selection.
fn bug_column_spec(action: &BugAction) -> Option<ColumnSpec<'_>> {
    let (BugAction::List { field_args, .. }
    | BugAction::My { field_args, .. }
    | BugAction::Search { field_args, .. }
    | BugAction::View { field_args, .. }) = action
    else {
        return None;
    };
    Some(ColumnSpec::new(
        field_args.fields.as_deref(),
        field_args.exclude_fields.as_deref(),
    ))
}

/// Dispatch bug actions to their respective handlers.
pub async fn execute(
    action: &BugAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    update::validate_action(action)?;

    // Resolve the field selection before any network I/O. A selection that
    // leaves nothing to show exits 7 deterministically — measured against the
    // five-column default in table mode, against the full field universe in
    // JSON mode (where output is trimmed gh-style to the selected keys). Either
    // way the exit code is independent of subcommand or server reachability.
    // `bug view` is exempt from this zero-field error in both modes: a sparse
    // (or empty `{}`) single-bug result is coherent, not an error. Unknown
    // `--fields` tokens (typos, custom `cf_*` fields) warn once on stderr;
    // under JSON the warning fires for every action including `view`, since a
    // typo would otherwise silently yield `{}`.
    if let Some(spec) = bug_column_spec(action) {
        let is_view = matches!(action, BugAction::View { .. });
        match format {
            OutputFormat::Table => {
                if !is_view {
                    validate_table_columns(spec)?;
                }
            }
            OutputFormat::Json => {
                if !is_view {
                    validate_json_field_selection(spec)?;
                }
                warn_unknown_fields(spec, w.err);
            }
        }
    }

    // Search builds its own client because --from-url may resolve a different
    // server from the URL hostname. Skip the shared connect to avoid double
    // auth/version detection on every `bug search` invocation.
    if let BugAction::Search { .. } = action {
        return search::handle(action, server, format, api, w).await;
    }

    let client = crate::commands::shared::connect_and_configure(server, api).await?;

    match action {
        BugAction::List { .. } => list::handle(&client, action, format, w).await,
        BugAction::View { .. } => view::handle(&client, action, format, w).await,
        BugAction::History { .. } => history::handle(&client, action, format, w).await,
        BugAction::My { .. } => my::handle(&client, action, format, w).await,
        BugAction::Create { .. } => create::handle(&client, action, format, w).await,
        BugAction::Clone { .. } => clone::handle(&client, action, format, w).await,
        BugAction::Update { .. } => update::handle(&client, action, format, w).await,
        BugAction::Search { .. } => unreachable!("handled above"),
    }
}
