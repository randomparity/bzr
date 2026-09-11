use crate::commands::runtime::invocation::CommandContext;
use crate::error::Result;
use crate::output::escape_terminal_controls;
use crate::output::result_types::{write_result, SearchResult};
use crate::output::writers::Writers;

pub(super) async fn handle(query: &str, ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let tags = client.search_comment_tags(query).await?;
    write_result(
        &SearchResult::new(tags.clone()),
        // Each tag is server-controlled text composed into a `write_result` table
        // listing, which the seam prints verbatim. Escape per tag so the newline that
        // separates the listing's rows stays a real newline (ADR 0070).
        &if tags.is_empty() {
            "No tags.".to_string()
        } else {
            tags.iter()
                .map(|tag| format!("  {}", escape_terminal_controls(tag)))
                .collect::<Vec<_>>()
                .join("\n")
        },
        ctx.format(),
        w.out,
    );
    Ok(())
}

#[cfg(test)]
#[path = "search_tags_tests.rs"]
mod tests;
