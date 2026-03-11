use std::io::Write;

use crate::cli::CommentAction;
use crate::client::BugzillaClient;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::{self, OutputFormat};

pub async fn execute(
    action: &CommentAction,
    server: Option<&str>,
    format: &OutputFormat,
) -> Result<()> {
    let config = Config::load()?;
    let srv = config.active_server(server)?;
    let client = BugzillaClient::new(&srv.url, &srv.api_key)?;

    match action {
        CommentAction::List { bug_id } => {
            let comments = client.get_comments(*bug_id).await?;
            output::print_comments(&comments, format)?;
        }
        CommentAction::Add { bug_id, body } => {
            let text = match body {
                Some(t) => t.clone(),
                None => edit_comment()?,
            };
            if text.trim().is_empty() {
                return Err(BzrError::Other("empty comment, aborting".into()));
            }
            let id = client.add_comment(*bug_id, &text).await?;
            println!("Added comment #{} to bug #{}", id, bug_id);
        }
    }
    Ok(())
}

fn edit_comment() -> Result<String> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    let mut tmpfile = tempfile()?;
    let path = tmpfile.path.clone();
    writeln!(tmpfile.file, "<!-- Enter your comment above this line -->")?;
    drop(tmpfile.file);

    let status = std::process::Command::new(&editor)
        .arg(&path)
        .status()?;

    if !status.success() {
        return Err(BzrError::Other(format!("{} exited with error", editor)));
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
