use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::attachment::write_attachments;
use crate::output::writers::Writers;
use crate::types::common::OutputFormat;

pub(super) async fn handle(
    ctx: &CommandContext,
    bug_id: u64,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let attachments = client.get_attachments(bug_id).await?;
    write_attachments(&attachments, format, w.out);
    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
