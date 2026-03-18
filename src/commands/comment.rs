use std::io::{IsTerminal, Read, Write};

use crate::cli::CommentAction;
use crate::error::{BzrError, Result};
use crate::output::{self, OutputFormat};

pub async fn execute(
    action: &CommentAction,
    server: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let client = super::shared::build_client(server).await?;

    match action {
        CommentAction::List { bug_id, since } => {
            let comments = client.get_comments_since(*bug_id, since.as_deref()).await?;
            output::print_comments(&comments, format);
        }
        CommentAction::Add { bug_id, body } => {
            let text = match body {
                Some(t) => t.clone(),
                None => read_comment_body()?,
            };
            if text.trim().is_empty() {
                return Err(BzrError::Other("empty comment, aborting".into()));
            }
            let id = client.add_comment(*bug_id, &text).await?;
            output::print_result(
                &serde_json::json!({
                    "id": id,
                    "bug_id": bug_id,
                    "resource": "comment",
                    "action": "created",
                }),
                &format!("Added comment #{id} to bug #{bug_id}"),
                format,
            );
        }
        CommentAction::Tag {
            comment_id,
            add,
            remove,
        } => {
            let tags = client.update_comment_tags(*comment_id, add, remove).await?;
            output::print_result(
                &serde_json::json!({
                    "comment_id": comment_id,
                    "tags": tags,
                    "resource": "comment_tag",
                    "action": "updated",
                }),
                &format!(
                    "Tags on comment #{comment_id}: {}",
                    if tags.is_empty() {
                        "(none)".to_string()
                    } else {
                        tags.join(", ")
                    }
                ),
                format,
            );
        }
        CommentAction::SearchTags { query } => {
            let tags = client.search_comment_tags(query).await?;
            output::print_comment_tags(&tags);
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
    edit_comment()
}

fn edit_comment() -> Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    let mut tmpfile = tempfile()?;
    let path = tmpfile.path.clone();
    writeln!(tmpfile.file, "<!-- Enter your comment above this line -->")?;
    drop(tmpfile.file);

    let status = std::process::Command::new(&editor).arg(&path).status()?;

    if !status.success() {
        return Err(BzrError::Other(format!("{editor} exited with error")));
    }

    let content = std::fs::read_to_string(&path)?;
    let _ = std::fs::remove_file(&path);

    let text: String = content
        .lines()
        .filter(|l| !l.starts_with("<!--"))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(text)
}

struct TempFile {
    path: std::path::PathBuf,
    file: std::fs::File,
}

fn tempfile() -> Result<TempFile> {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("bzr-comment-{}.txt", std::process::id()));
    let file = std::fs::File::create(&path)?;
    Ok(TempFile { path, file })
}
