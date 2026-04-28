//! Bug subcommand handlers, split per-action.

use crate::cli::BugAction;
use crate::error::Result;
use crate::types::{ApiMode, OutputFormat};

mod clone;
mod create;
mod history;
mod list;
mod my;
mod search;
mod shared;
mod update;
mod view;

/// Dispatch bug actions to their respective handlers.
pub async fn execute(
    action: &BugAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = crate::commands::shared::connect_and_configure(server, api).await?;

    match action {
        BugAction::List { .. } => list::handle(&client, action, format).await,
        BugAction::View { .. } => view::handle(&client, action, format).await,
        BugAction::History { .. } => history::handle(&client, action, format).await,
        BugAction::Search { .. } => search::handle(action, server, format, api).await,
        BugAction::My { .. } => my::handle(&client, action, format).await,
        BugAction::Create { .. } => create::handle(&client, action, format).await,
        BugAction::Clone { .. } => clone::handle(&client, action, format).await,
        BugAction::Update { .. } => update::handle(&client, action, format).await,
    }
}
