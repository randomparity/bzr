//! Bug subcommand handlers, split per-action.

use crate::cli::BugAction;
use crate::error::Result;
use crate::output::resources::bug::{
    validate_table_columns, warn_json_field_selection, ColumnSpec,
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
    match action {
        BugAction::List {
            fields,
            exclude_fields,
            ..
        }
        | BugAction::My {
            fields,
            exclude_fields,
            ..
        }
        | BugAction::Search {
            fields,
            exclude_fields,
            ..
        }
        | BugAction::View {
            fields,
            exclude_fields,
            ..
        } => Some(ColumnSpec {
            include: fields.as_deref(),
            exclude: exclude_fields.as_deref(),
        }),
        _ => None,
    }
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

    // Resolve the field selection before any network I/O. In JSON mode, warn
    // that `--fields`/`--exclude-fields` restricts what is *fetched*, not what
    // is shown (unselected fields come back null/empty). In table mode, a
    // zero-column selection exits 7 deterministically, regardless of subcommand
    // or server reachability. `bug view` is exempt from table validation: a
    // header-only detail block is a coherent sparse result, not an error.
    if let Some(spec) = bug_column_spec(action) {
        if format == OutputFormat::Json {
            warn_json_field_selection(spec, w.err);
        } else if !matches!(action, BugAction::View { .. }) {
            validate_table_columns(spec)?;
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
