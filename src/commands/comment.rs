use std::io::{IsTerminal, Read, Write};

use crate::cli::CommentAction;
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
            let comments = client.get_comments_since(*bug_id, since.as_deref()).await?;
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
    compose_comment_in_editor()
}

fn compose_comment_in_editor() -> Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    let tmpfile = create_comment_tempfile("<!-- Enter your comment above this line -->\n")?;

    let status = std::process::Command::new(&editor)
        .arg(&tmpfile.path)
        .status()?;

    if !status.success() {
        // TempFile::drop cleans up the file
        return Err(BzrError::InputValidation(format!(
            "{editor} exited with error"
        )));
    }

    let content = std::fs::read_to_string(&tmpfile.path)?;
    // TempFile::drop cleans up the file

    Ok(filter_comment_body(&content))
}

/// Strip HTML comment lines (editor instructions) from raw comment text.
fn filter_comment_body(raw: &str) -> String {
    raw.lines()
        .filter(|l| !l.starts_with("<!--"))
        .collect::<Vec<_>>()
        .join("\n")
}

struct TempFile {
    path: std::path::PathBuf,
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.path) {
            tracing::debug!(path = %self.path.display(), "failed to remove temp file: {e}");
        }
    }
}

fn create_comment_tempfile(initial_content: &str) -> Result<TempFile> {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("bzr-comment-{}.txt", std::process::id()));
    let mut file = std::fs::File::create(&path)?;
    file.write_all(initial_content.as_bytes())?;
    drop(file);
    Ok(TempFile { path })
}

#[cfg(test)]
#[path = "comment_tests.rs"]
mod tests;
