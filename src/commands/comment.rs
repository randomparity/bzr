use std::io::{IsTerminal, Read, Write};

use crate::cli::CommentAction;
use crate::error::{BzrError, Result};
use crate::output::{self, ActionResult, ResourceKind};
use crate::types::ApiMode;
use crate::types::OutputFormat;

pub async fn execute(
    action: &CommentAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_client(server, api).await?;

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
                return Err(BzrError::InputValidation("empty comment, aborting".into()));
            }
            let id = client.add_comment(*bug_id, &text).await?;
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
            output::print_result(
                &serde_json::json!(tags),
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
    edit_comment()
}

fn edit_comment() -> Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    let mut tmpfile = tempfile()?;
    if let Some(ref mut f) = tmpfile.file {
        writeln!(f, "<!-- Enter your comment above this line -->")?;
    }
    tmpfile.close_file();

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

    let text: String = content
        .lines()
        .filter(|l| !l.starts_with("<!--"))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(text)
}

struct TempFile {
    path: std::path::PathBuf,
    file: Option<std::fs::File>,
}

impl TempFile {
    /// Close the file handle (flushes writes) while keeping the path for reading.
    fn close_file(&mut self) {
        self.file.take();
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn tempfile() -> Result<TempFile> {
    let dir = std::env::temp_dir();
    let path = dir.join(format!("bzr-comment-{}.txt", std::process::id()));
    let file = std::fs::File::create(&path)?;
    Ok(TempFile {
        path,
        file: Some(file),
    })
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::super::test_helpers::{setup_config, ENV_LOCK};
    use crate::cli::CommentAction;
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn comment_list_returns_comments() {
        let _lock = ENV_LOCK.lock().await;
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        Mock::given(method("GET"))
            .and(path("/rest/bug/42/comment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "bugs": {
                    "42": {
                        "comments": [{
                            "id": 1,
                            "bug_id": 42,
                            "text": "Hello world",
                            "creator": "user@test.com",
                            "creation_time": "2025-01-01T00:00:00Z",
                            "is_private": false,
                            "count": 0
                        }]
                    }
                }
            })))
            .mount(&mock)
            .await;

        let action = CommentAction::List {
            bug_id: 42,
            since: None,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn comment_add_with_body() {
        let _lock = ENV_LOCK.lock().await;
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        Mock::given(method("POST"))
            .and(path("/rest/bug/42/comment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"id": 100})))
            .mount(&mock)
            .await;

        let action = CommentAction::Add {
            bug_id: 42,
            body: Some("Test comment".to_string()),
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn comment_add_empty_body_is_rejected() {
        let _lock = ENV_LOCK.lock().await;
        let mock = MockServer::start().await;
        let tmp = tempfile::TempDir::new().unwrap();
        setup_config(&tmp, &mock.uri());

        // No mock needed — execute() should reject before making any API call
        let action = CommentAction::Add {
            bug_id: 42,
            body: Some("   ".to_string()),
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err(), "empty body should be rejected");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("empty comment"),
            "expected 'empty comment' error, got: {err}"
        );
    }
}
