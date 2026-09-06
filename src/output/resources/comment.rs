use std::io::Write;

use colored::Colorize;

use crate::output::formatting::{escape_table_control, write_divider, write_formatted_projected};
use crate::types::comment::Comment;
use crate::types::output::OutputFormat;
use crate::validation::fields::FieldProjection;

pub fn write_comments<W: Write + ?Sized>(
    comments: &[Comment],
    format: OutputFormat,
    projection: &FieldProjection,
    out: &mut W,
) {
    write_formatted_projected(comments, format, projection, out, |comments, out| {
        if comments.is_empty() {
            let _ = writeln!(out, "No comments.");
            return;
        }
        for c in comments {
            let count = c
                .count
                .map_or_else(|| "?".to_string(), |value| value.to_string());
            let _ = writeln!(
                out,
                "{} #{} by {} ({})",
                "Comment".bold(),
                count,
                c.creator.as_deref().unwrap_or("unknown").cyan(),
                c.creation_time.as_deref().unwrap_or(""),
            );
            if c.is_private.unwrap_or(false) {
                let _ = writeln!(out, "  {}", "[PRIVATE]".red());
            }
            if !c.tags.is_empty() {
                let tags = c
                    .tags
                    .iter()
                    .map(|tag| escape_table_control(tag))
                    .collect::<Vec<_>>()
                    .join(", ");
                let _ = writeln!(out, "  {} {tags}", "Tags:".bold());
            }
            let _ = writeln!(out);
            for line in c.text.as_deref().unwrap_or("").lines() {
                let _ = writeln!(out, "  {line}");
            }
            let _ = writeln!(out);
            write_divider(out);
        }
    });
}

/// Header separating one bug's comments from the next in multi-ID table
/// output. JSON stays a flat array attributed by each record's `bug_id`.
pub fn write_comment_bug_header<W: Write + ?Sized>(bug_id: u64, out: &mut W) {
    // Every other table writer reaches `colored`'s test seam through the
    // `write_formatted*` family; this one writes directly, so it calls the seam
    // itself. Without it, `colored` would emit `ESC[1mBugESC[0m #42` and split
    // the `Bug #42` substring the tests match on.
    crate::output::formatting::disable_color_for_tests();
    let _ = writeln!(out, "{} #{}", "Bug".bold(), bug_id);
    let _ = writeln!(out);
}

#[cfg(test)]
#[path = "comment_tests.rs"]
mod tests;
