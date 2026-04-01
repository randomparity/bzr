//! Saved query management commands.
//!
//! Query operations are pure local file I/O — no network client needed.

use crate::cli::QueryAction;
use crate::error::{BzrError, Result};
use crate::types::OutputFormat;

#[expect(
    clippy::unused_async,
    reason = "async for signature consistency with sibling execute fns"
)]
pub async fn execute(
    action: &QueryAction,
    _server: Option<&str>,
    _format: OutputFormat,
    _api: Option<crate::types::ApiMode>,
) -> Result<()> {
    match action {
        QueryAction::Save { .. }
        | QueryAction::List
        | QueryAction::Show { .. }
        | QueryAction::Delete { .. }
        | QueryAction::Run { .. } => {
            Err(BzrError::Other("query command not yet implemented".into()))
        }
    }
}
