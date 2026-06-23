use std::io::IsTerminal;

use crate::cli::CommentAction;
use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::editor;
use crate::error::{BzrError, Result};
use crate::output::resources::comment::write_comments;
use crate::output::result_types::{
    write_result, ActionResult, ResourceKind, SearchResult, TagResult,
};
use crate::output::writers::Writers;
use crate::types::{AddCommentParams, UpdateCommentTagsParams};

pub(crate) fn requires_credentials(action: &CommentAction) -> Option<&'static str> {
    match action {
        CommentAction::List { .. } | CommentAction::SearchTags { .. } => None,
        CommentAction::Add { .. } => Some("comment add"),
        CommentAction::Tag { .. } => Some("comment tag"),
    }
}

pub async fn execute(
    action: &CommentAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    validate_action(action)?;
    let format = ctx.format();
    let client = super::runtime::shared::connect_and_configure(ctx).await?;

    match action {
        CommentAction::List { bug_id, since } => {
            let canonical_since =
                crate::validation::parse_optional_date(since.as_deref(), "--since")?;
            let comments = client
                .get_comments_since(*bug_id, canonical_since.as_deref())
                .await?;
            write_comments(&comments, format, w.out);
        }
        CommentAction::Add {
            bug_id,
            body,
            body_file,
            private,
        } => {
            let text = super::runtime::shared::materialize_comment_body(
                super::runtime::shared::classify_body_source(
                    body.as_deref(),
                    body_file.as_deref(),
                    "--body",
                    "--body-file",
                )?,
                "--body-file",
                super::runtime::shared::CommentBodyRequirement::RequiredWithFallback(
                    read_comment_body,
                ),
            )?;
            let Some(text) = text else {
                return Err(BzrError::DataIntegrity(
                    "comment body requirement did not produce a body".into(),
                ));
            };
            let params = AddCommentParams {
                text,
                is_private: *private,
            };
            let id = client.add_comment(*bug_id, &params).await?;
            write_result(
                &ActionResult::created(id, ResourceKind::Comment),
                &format!("Added comment #{id} to bug #{bug_id}"),
                format,
                w.out,
            );
        }
        CommentAction::Tag {
            comment_id,
            add,
            remove,
        } => {
            let params = UpdateCommentTagsParams {
                add: add.clone(),
                remove: remove.clone(),
            };
            let tags = client.update_comment_tags(*comment_id, &params).await?;
            let display = if tags.is_empty() {
                "(none)".to_string()
            } else {
                tags.join(", ")
            };
            write_result(
                &TagResult::updated(*comment_id, tags),
                &format!("Tags on comment #{comment_id}: {display}"),
                format,
                w.out,
            );
        }
        CommentAction::SearchTags { query } => {
            let tags = client.search_comment_tags(query).await?;
            write_result(
                &SearchResult::new(tags.clone()),
                &if tags.is_empty() {
                    "No tags.".to_string()
                } else {
                    tags.iter()
                        .map(|t| format!("  {t}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                },
                format,
                w.out,
            );
        }
    }
    Ok(())
}

fn validate_action(action: &CommentAction) -> Result<()> {
    match action {
        CommentAction::Tag { add, remove, .. } if add.is_empty() && remove.is_empty() => Err(
            BzrError::InputValidation("no comment tag changes; specify --add or --remove".into()),
        ),
        _ => Ok(()),
    }
}

/// Read comment body from stdin (pipe) or $EDITOR (TTY).
fn read_comment_body() -> Result<String> {
    if !std::io::stdin().is_terminal() {
        return super::runtime::shared::read_stdin_to_string("read comment body from stdin");
    }
    let raw = editor::launch("<!-- Enter your comment above this line -->\n", "comment")?;
    Ok(filter_comment_body(&raw))
}

/// Strip HTML comment lines (editor instructions) from raw comment text.
fn filter_comment_body(raw: &str) -> String {
    raw.lines()
        .filter(|l| !l.starts_with("<!--"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[path = "comment_tests.rs"]
mod tests;
