//! Attachment subcommand handlers, split per action.

use crate::cli::AttachmentAction;
use crate::commands::runtime::invocation::CommandCapabilities;
use crate::commands::runtime::invocation::CommandContext;
use crate::error::Result;
use crate::output::writers::Writers;

mod download;
mod list;
mod update;
mod upload;
mod view;

#[must_use]
pub(crate) fn capabilities(action: &AttachmentAction) -> CommandCapabilities {
    match action {
        AttachmentAction::List { .. }
        | AttachmentAction::View { .. }
        | AttachmentAction::Download { .. } => CommandCapabilities::anonymous(),
        AttachmentAction::Upload(_) => CommandCapabilities::authenticated("attachment upload"),
        AttachmentAction::Update(_) => CommandCapabilities::authenticated("attachment update"),
    }
}

/// Collapse a `--flag` / `--no-flag` presence pair into a tri-state.
///
/// Returns `Some(true)` for the positive flag, `Some(false)` for the negative,
/// and `None` when neither is given (caller decides the unset meaning). clap's
/// `overrides_with` guarantees the two are never both `true`, so the order of
/// the checks is irrelevant.
pub(super) fn resolve_bool_flag(yes: bool, no: bool) -> Option<bool> {
    if yes {
        Some(true)
    } else if no {
        Some(false)
    } else {
        None
    }
}

pub(crate) async fn execute(
    action: &AttachmentAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    validate_action(action)?;
    let format = ctx.format();

    match action {
        AttachmentAction::List { bug_id, projection } => {
            list::handle(ctx, *bug_id, format, projection, w).await?;
        }
        AttachmentAction::View { attachment_id } => {
            view::handle(ctx, *attachment_id, format, w).await?;
        }
        AttachmentAction::Download {
            ids,
            bug_ids,
            out,
            out_dir,
        } => {
            download::handle(
                download::DownloadArgs {
                    ids,
                    bug_ids,
                    out: out.as_deref(),
                    out_dir,
                },
                ctx,
                format,
                w,
            )
            .await?;
        }
        AttachmentAction::Upload(args) => upload::handle(args, ctx, format, w).await?,
        AttachmentAction::Update(args) => update::handle(args, ctx, format, w).await?,
    }
    Ok(())
}

fn validate_action(action: &AttachmentAction) -> Result<()> {
    match action {
        AttachmentAction::Upload(crate::cli::UploadArgs {
            comment_private: true,
            comment: None,
            comment_file: None,
            ..
        }) => Err(crate::error::BzrError::InputValidation(
            "--comment-private requires --comment or --comment-file".into(),
        )),
        AttachmentAction::Download { ids, bug_ids, .. } if ids.is_empty() && bug_ids.is_empty() => {
            Err(crate::error::BzrError::InputValidation(
                "specify at least one attachment ID or --bug <ID>".into(),
            ))
        }
        AttachmentAction::Download {
            ids, out: Some(_), ..
        } if ids.len() != 1 => Err(crate::error::BzrError::InputValidation(
            "--out requires exactly one attachment ID".into(),
        )),
        AttachmentAction::Update(args) if !update_has_changes(args) => {
            Err(crate::error::BzrError::InputValidation(
                "no attachment fields to update; specify at least one of --summary, --file-name, \
                 --content-type, --obsolete/--no-obsolete, --patch/--no-patch, \
                 --private/--no-private, or --flag"
                    .into(),
            ))
        }
        _ => Ok(()),
    }
}

fn update_has_changes(args: &crate::cli::AttachmentUpdateArgs) -> bool {
    args.summary.is_some()
        || args.file_name.is_some()
        || args.content_type.is_some()
        || args.obsolete
        || args.no_obsolete
        || args.patch
        || args.no_patch
        || args.private
        || args.no_private
        || !args.flag.is_empty()
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
