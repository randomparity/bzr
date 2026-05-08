//! Bug subcommand handlers, split per-action.

use crate::cli::BugAction;
use crate::error::Result;
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

/// Dispatch bug actions to their respective handlers.
pub async fn execute(
    action: &BugAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
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
