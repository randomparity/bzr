use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::attachment::write_attachment;
use crate::output::writers::Writers;
use crate::types::common::OutputFormat;

pub(super) async fn handle(
    ctx: &CommandContext,
    attachment_id: u64,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let attachment = client.get_attachment_metadata(attachment_id).await?;
    write_attachment(&attachment, format, w.out);
    Ok(())
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
