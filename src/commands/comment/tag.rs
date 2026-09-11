use crate::commands::runtime::invocation::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::escape_terminal_controls;
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
        return Err(BzrError::input(
            "no comment tag changes; specify --add or --remove".into(),
        ));
    }

    let params = UpdateCommentTagsParams {
        add: add.to_vec(),
        remove: remove.to_vec(),
    };
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let tags = client.update_comment_tags(comment_id, &params).await?;
    // The server echoes the resulting tag set, so each tag is server-controlled text
    // composed into a `write_result` table line — a seam that prints its message
    // verbatim. Escape per tag so the `, ` separator stays a real separator (ADR 0070).
    let display = if tags.is_empty() {
        "(none)".to_string()
    } else {
        tags.iter()
            .map(|tag| escape_terminal_controls(tag))
            .collect::<Vec<_>>()
            .join(", ")
    };
    write_result(
        &TagResult::updated(comment_id, tags),
        &format!("Tags on comment #{comment_id}: {display}"),
        ctx.format(),
        w.out,
    );
    Ok(())
}

#[cfg(test)]
#[path = "tag_tests.rs"]
mod tests;
