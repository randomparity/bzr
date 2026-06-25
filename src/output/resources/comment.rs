use std::io::Write;

use colored::Colorize;

use crate::output::formatting::{write_divider, write_formatted};
use crate::types::comment::Comment;
use crate::types::output::OutputFormat;

pub fn write_comments<W: Write + ?Sized>(comments: &[Comment], format: OutputFormat, out: &mut W) {
    write_formatted(comments, format, out, |comments, out| {
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
            let _ = writeln!(out);
            for line in c.text.as_deref().unwrap_or("").lines() {
                let _ = writeln!(out, "  {line}");
            }
            let _ = writeln!(out);
            write_divider(out);
        }
    });
}

#[cfg(test)]
#[path = "comment_tests.rs"]
mod tests;
