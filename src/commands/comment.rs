use std::io::{IsTerminal, Read};

use crate::cli::CommentAction;
use crate::commands::editor;
use crate::error::{BzrError, Result};
use crate::output::{self, ActionResult, ResourceKind, SearchResult, TagResult};
use crate::types::ApiMode;
use crate::types::{OutputFormat, UpdateCommentTagsParams};

pub async fn execute(
    action: &CommentAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_and_configure(server, api).await?;

    match action {
        CommentAction::List { bug_id, since } => {
            let canonical_since = since
                .as_deref()
                .map(|s| crate::validation::parse_iso8601_or_date(s, "--since"))
                .transpose()?;
            let comments = client
                .get_comments_since(*bug_id, canonical_since.as_deref())
                .await?;
            output::print_comments(&comments, format);
        }
        CommentAction::Add {
            bug_id,
            body,
            private,
        } => {
            let text = match body {
                Some(t) => t.clone(),
                None => read_comment_body()?,
            };
            if text.trim().is_empty() {
                return Err(BzrError::InputValidation("empty comment, aborting".into()));
            }
            let id = client.add_comment(*bug_id, &text, *private).await?;
            output::print_result(
                &ActionResult::created(id, ResourceKind::Comment),
                &format!("Added comment #{id} to bug #{bug_id}"),
                format,
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
            output::print_result(
                &TagResult::updated(*comment_id, tags),
                &format!("Tags on comment #{comment_id}: {display}"),
                format,
            );
        }
        CommentAction::SearchTags { query } => {
            let tags = client.search_comment_tags(query).await?;
            output::print_result(
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
            );
        }
    }
    Ok(())
}

/// Read comment body from stdin (pipe) or $EDITOR (TTY).
fn read_comment_body() -> Result<String> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        let mut buf = String::new();
        stdin.lock().read_to_string(&mut buf)?;
        return Ok(buf);
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
