use std::io::{self, Write};

use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output;
use crate::types::OutputFormat;
use crate::validation::parse_optional_date;

pub(super) async fn handle(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let BugAction::History { id, since } = action else {
        unreachable!()
    };

    let canonical_since = parse_optional_date(since.as_deref(), "--since")?;

    let history = client
        .get_bug_history_since(*id, canonical_since.as_deref())
        .await?;
    if history.is_empty() {
        let _ = writeln!(io::stdout(), "No history for bug #{id}.");
    } else {
        output::print_history(&history, format);
    }
    Ok(())
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
