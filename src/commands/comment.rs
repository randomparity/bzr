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
        let _ = std::fs::remove_file(&self.path);
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
#[expect(clippy::unwrap_used)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    use super::super::test_helpers::{capture_stdout, setup_test_env};
    use crate::cli::CommentAction;
    use crate::types::OutputFormat;

    #[tokio::test]
    async fn comment_list_returns_comments() {
        let (_lock, mock, _tmp) = setup_test_env().await;

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
        let (result, output) =
            capture_stdout(super::execute(&action, None, OutputFormat::Json, None)).await;
        assert!(result.is_ok());
        let parsed: serde_json::Value = super::super::test_helpers::extract_json(&output);
        assert_eq!(parsed[0]["id"], 1);
        assert_eq!(parsed[0]["text"], "Hello world");
        assert_eq!(parsed[0]["creator"], "user@test.com");
    }

    #[tokio::test]
    async fn comment_add_with_body() {
        let (_lock, mock, _tmp) = setup_test_env().await;

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
        let (_lock, _mock, _tmp) = setup_test_env().await;

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

    #[test]
    fn filter_comment_body_strips_html_comments() {
        let raw = "Hello\n<!-- Enter your comment above this line -->\nWorld";
        assert_eq!(super::filter_comment_body(raw), "Hello\nWorld");
    }

    #[test]
    fn filter_comment_body_preserves_normal_text() {
        let raw = "Just a comment\nwith multiple lines";
        assert_eq!(super::filter_comment_body(raw), raw);
    }

    #[test]
    fn filter_comment_body_empty_input() {
        assert_eq!(super::filter_comment_body(""), "");
    }

    #[tokio::test]
    async fn comment_list_http_500_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("GET"))
            .and(path("/rest/bug/42/comment"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&mock)
            .await;

        let action = CommentAction::List {
            bug_id: 42,
            since: None,
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("500") || err.contains("Internal Server Error"),
            "expected HTTP 500 error, got: {err}"
        );
    }

    #[tokio::test]
    async fn comment_add_api_error_returns_error() {
        let (_lock, mock, _tmp) = setup_test_env().await;

        Mock::given(method("POST"))
            .and(path("/rest/bug/42/comment"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": true,
                "code": 100,
                "message": "Bug #42 does not exist."
            })))
            .mount(&mock)
            .await;

        let action = CommentAction::Add {
            bug_id: 42,
            body: Some("Test comment".to_string()),
        };
        let result = super::execute(&action, None, OutputFormat::Json, None).await;
        assert!(result.is_err());
    }
}
