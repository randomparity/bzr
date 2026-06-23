use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::resources::attachment::write_attachments;
use crate::output::writers::Writers;
use crate::types::OutputFormat;

pub(super) async fn handle(
    client: &BugzillaClient,
    bug_id: u64,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let attachments = client.get_attachments(bug_id).await?;
    write_attachments(&attachments, format, w.out);
    Ok(())
}
