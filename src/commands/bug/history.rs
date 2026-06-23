use crate::cli::HistoryArgs;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::resources::bug::write_history;
use crate::output::writers::Writers;
use crate::types::common::OutputFormat;
use crate::validation::parse_optional_date;

pub(super) async fn handle(
    client: &BugzillaClient,
    args: &HistoryArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let HistoryArgs { id, since } = args;

    let canonical_since = parse_optional_date(since.as_deref(), "--since")?;

    let history = client
        .get_bug_history_since(*id, canonical_since.as_deref())
        .await?;
    if history.is_empty() {
        let _ = writeln!(w.out, "No history for bug #{id}.");
    } else {
        write_history(&history, format, w.out);
    }
    Ok(())
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
