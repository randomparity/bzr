use std::io::IsTerminal;

use crate::cli::CommentAction;
use crate::commands::runtime::interaction::editor;
use crate::commands::runtime::invocation::CommandCapabilities;
use crate::commands::runtime::invocation::CommandContext;
use crate::error::Result;
use crate::output::writers::Writers;

mod add;
mod list;
mod search_tags;
mod tag;

#[must_use]
pub(crate) fn capabilities(action: &CommentAction) -> CommandCapabilities {
    match action {
        CommentAction::List { .. } | CommentAction::SearchTags { .. } => {
            CommandCapabilities::anonymous()
        }
        CommentAction::Add { .. } => CommandCapabilities::authenticated("comment add"),
        CommentAction::Tag { .. } => CommandCapabilities::authenticated("comment tag"),
    }
}

pub(crate) async fn execute(
    action: &CommentAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        CommentAction::List {
            bug_ids,
            permissive,
            since,
            projection,
        } => {
            let args = list::ListArgs {
                bug_ids,
                permissive: *permissive,
                since: since.as_deref(),
                projection,
            };
            list::handle(&args, ctx, w).await
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
