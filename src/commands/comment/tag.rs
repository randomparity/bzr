use crate::commands::runtime::context::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::result_types::{write_result, TagResult};
use crate::output::writers::Writers;
use crate::types::comment::UpdateCommentTagsParams;

pub(super) async fn handle(
    comment_id: u64,
    add: &[String],
    remove: &[String],
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    if add.is_empty() && remove.is_empty() {
        return Err(BzrError::InputValidation(
            "no comment tag changes; specify --add or --remove".into(),
        ));
    }

    let params = UpdateCommentTagsParams {
        add: add.to_vec(),
        remove: remove.to_vec(),
    };
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let tags = client.update_comment_tags(comment_id, &params).await?;
    let display = if tags.is_empty() {
        "(none)".to_string()
    } else {
        tags.join(", ")
    };
    write_result(
        &TagResult::updated(comment_id, tags),
        &format!("Tags on comment #{comment_id}: {display}"),
        ctx.format(),
        w.out,
    );
    Ok(())
}
