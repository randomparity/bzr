use colored::Colorize;

use super::print_formatted;
use crate::types::{Comment, OutputFormat};

#[expect(clippy::print_stdout)]
pub fn print_comments(comments: &[Comment], format: OutputFormat) {
    print_formatted(comments, format, |comments| {
        if comments.is_empty() {
            println!("No comments.");
            return;
        }
        for c in comments {
            println!(
                "{} #{} by {} ({})",
                "Comment".bold(),
                c.count,
                c.creator.as_deref().unwrap_or("unknown").cyan(),
                c.creation_time.as_deref().unwrap_or(""),
            );
            if c.is_private {
                println!("  {}", "[PRIVATE]".red());
            }
            println!();
            for line in c.text.lines() {
                println!("  {line}");
            }
            println!();
            println!("{}", "─".repeat(60));
        }
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use crate::types::Comment;

    fn make_comment(count: u64, text: &str) -> Comment {
        Comment {
            id: count + 100,
            bug_id: 42,
            text: text.into(),
            creator: Some("commenter@example.com".into()),
            creation_time: Some("2025-02-01T08:00:00Z".into()),
            count,
            is_private: false,
        }
    }

    #[test]
    fn print_comments_json_empty() {
        let comments: Vec<Comment> = vec![];
        let json = serde_json::to_string_pretty(&comments).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn print_comments_json_one_comment() {
        let comments = vec![make_comment(0, "First comment text")];
        let json = serde_json::to_string_pretty(&comments).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["text"], "First comment text");
        assert_eq!(parsed[0]["count"], 0);
        assert_eq!(parsed[0]["creator"], "commenter@example.com");
    }

    #[test]
    fn print_comments_json_private_flag() {
        let mut comment = make_comment(1, "secret");
        comment.is_private = true;
        let json = serde_json::to_string(&comment).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["is_private"], true);
    }
}
