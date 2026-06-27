use crate::commands::runtime::invocation::CommandContext;
use crate::error::Result;
use crate::output::result_types::{write_result, ActionResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::attachment::UpdateAttachmentParams;
use crate::types::output::OutputFormat;

pub(super) async fn handle(
    args: &crate::cli::AttachmentUpdateArgs,
    ctx: &CommandContext,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let (id, params) = build_update_params(args)?;
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    client.update_attachment(id, &params).await?;
    write_result(
        &ActionResult::updated(id, ResourceKind::Attachment),
        &format!("Updated attachment #{id}"),
        format,
        w.out,
    );
    Ok(())
}

fn build_update_params(
    args: &crate::cli::AttachmentUpdateArgs,
) -> Result<(u64, UpdateAttachmentParams)> {
    let crate::cli::AttachmentUpdateArgs {
        id,
        summary,
        file_name,
        content_type,
        obsolete,
        no_obsolete,
        patch,
        no_patch,
        private,
        no_private,
        flag,
    } = args;
    let flags = crate::commands::runtime::input::flags::parse_flags(flag)?;
    let params = UpdateAttachmentParams {
        summary: summary.clone(),
        file_name: file_name.clone(),
        content_type: content_type.clone(),
        // None = leave unchanged; Some(b) = set explicitly.
        is_obsolete: super::resolve_bool_flag(*obsolete, *no_obsolete),
        is_patch: super::resolve_bool_flag(*patch, *no_patch),
        is_private: super::resolve_bool_flag(*private, *no_private),
        flags,
    };
    Ok((*id, params))
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
