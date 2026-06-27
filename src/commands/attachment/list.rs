use crate::commands::runtime::invocation::CommandContext;
use crate::error::Result;
use crate::output::resources::attachment::write_attachments;
use crate::output::writers::Writers;
use crate::types::output::OutputFormat;

pub(super) async fn handle(
    ctx: &CommandContext,
    bug_id: u64,
    format: OutputFormat,
    projection_args: &crate::cli::ProjectionArgs,
    w: &mut Writers<'_>,
) -> Result<()> {
    let projection = crate::validation::fields::projection_for(
        format,
        projection_args.fields.as_deref(),
        projection_args.exclude_fields.as_deref(),
        crate::types::attachment::ATTACHMENT_FIELDS,
        w.err,
    )?;
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let attachments = client.get_attachments(bug_id).await?;
    write_attachments(&attachments, format, &projection, w.out);
    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
