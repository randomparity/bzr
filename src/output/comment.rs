use std::io::{self, Write as _};

use colored::Colorize;

use super::formatting::print_formatted;
use crate::types::{Comment, OutputFormat};

pub fn print_comments(comments: &[Comment], format: OutputFormat) {
    print_formatted(comments, format, |comments| {
        if comments.is_empty() {
            let _ = writeln!(io::stdout(), "No comments.");
            return;
        }
        for c in comments {
            let _ = writeln!(
                io::stdout(),
                "{} #{} by {} ({})",
                "Comment".bold(),
                c.count,
                c.creator.as_deref().unwrap_or("unknown").cyan(),
                c.creation_time.as_deref().unwrap_or(""),
            );
            if c.is_private {
                let _ = writeln!(io::stdout(), "  {}", "[PRIVATE]".red());
            }
            let _ = writeln!(io::stdout());
            for line in c.text.lines() {
                let _ = writeln!(io::stdout(), "  {line}");
            }
            let _ = writeln!(io::stdout());
            let _ = writeln!(io::stdout(), "{}", "─".repeat(60));
        }
    });
}

#[cfg(test)]
#[path = "comment_tests.rs"]
mod tests;
