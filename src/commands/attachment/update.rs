use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::result_types::{write_result, ActionResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::{OutputFormat, UpdateAttachmentParams};

pub(super) async fn handle(
    client: &BugzillaClient,
    args: &crate::cli::AttachmentUpdateArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
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
    let flags = crate::commands::runtime::flags::parse_flags(flag)?;
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
    client.update_attachment(*id, &params).await?;
    write_result(
        &ActionResult::updated(*id, ResourceKind::Attachment),
        &format!("Updated attachment #{id}"),
        format,
        w.out,
    );
    Ok(())
}
