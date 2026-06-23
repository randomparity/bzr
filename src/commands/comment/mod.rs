use std::io::IsTerminal;

use crate::cli::CommentAction;
use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::editor;
use crate::error::Result;
use crate::output::writers::Writers;

mod add;
mod list;
mod search_tags;
mod tag;

pub(crate) fn requires_credentials(action: &CommentAction) -> Option<&'static str> {
    match action {
        CommentAction::List { .. } | CommentAction::SearchTags { .. } => None,
        CommentAction::Add { .. } => Some("comment add"),
        CommentAction::Tag { .. } => Some("comment tag"),
    }
}

pub(crate) async fn execute(
    action: &CommentAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        CommentAction::List { bug_id, since } => {
            list::handle(*bug_id, since.as_deref(), ctx, w).await
        }
        CommentAction::Add {
            bug_id,
            body,
            body_file,
            private,
        } => {
            let args = add::AddArgs {
                bug_id: *bug_id,
                body: body.as_deref(),
                body_file: body_file.as_deref(),
                private: *private,
            };
            add::handle(&args, ctx, w).await
        }
        CommentAction::Tag {
            comment_id,
            add,
            remove,
        } => tag::handle(*comment_id, add, remove, ctx, w).await,
        CommentAction::SearchTags { query } => search_tags::handle(query, ctx, w).await,
    }
}

/// Read comment body from stdin (pipe) or $EDITOR (TTY).
fn read_comment_body() -> Result<String> {
    if !std::io::stdin().is_terminal() {
        return crate::commands::runtime::shared::read_stdin_to_string(
            "read comment body from stdin",
        );
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
#[path = "mod_tests.rs"]
mod tests;
