use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::resources::attachment::write_attachment;
use crate::output::writers::Writers;
use crate::types::OutputFormat;

pub(super) async fn handle(
    client: &BugzillaClient,
    attachment_id: u64,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let attachment = client.get_attachment_metadata(attachment_id).await?;
    write_attachment(&attachment, format, w.out);
    Ok(())
}
