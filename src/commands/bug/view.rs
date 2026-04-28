use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output;
use crate::types::OutputFormat;

pub(super) async fn handle(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let BugAction::View {
        id,
        fields,
        exclude_fields,
    } = action
    else {
        unreachable!()
    };

    let bug = client
        .get_bug(id, fields.as_deref(), exclude_fields.as_deref())
        .await?;
    output::print_bug_detail(&bug, format);
    Ok(())
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
