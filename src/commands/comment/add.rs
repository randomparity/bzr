use std::path::Path;

use crate::commands::runtime::context::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::result_types::{write_result, ActionResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::AddCommentParams;

pub(super) struct AddArgs<'a> {
    pub(super) bug_id: u64,
    pub(super) body: Option<&'a str>,
    pub(super) body_file: Option<&'a Path>,
    pub(super) private: bool,
}

pub(super) async fn handle(
    args: &AddArgs<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let text = crate::commands::runtime::shared::materialize_comment_body(
        crate::commands::runtime::shared::classify_body_source(
            args.body,
            args.body_file,
            "--body",
            "--body-file",
        )?,
        "--body-file",
        crate::commands::runtime::shared::CommentBodyRequirement::RequiredWithFallback(
            super::read_comment_body,
        ),
    )?;
    let Some(text) = text else {
        return Err(BzrError::DataIntegrity(
            "comment body requirement did not produce a body".into(),
        ));
    };
    let params = AddCommentParams {
        text,
        is_private: args.private,
    };
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let id = client.add_comment(args.bug_id, &params).await?;
    write_result(
        &ActionResult::created(id, ResourceKind::Comment),
        &format!("Added comment #{id} to bug #{}", args.bug_id),
        ctx.format(),
        w.out,
    );
    Ok(())
}
