use colored::Colorize;

use super::print_formatted;
use crate::types::{Attachment, OutputFormat};

#[expect(clippy::print_stdout)]
pub fn print_attachments(attachments: &[Attachment], format: OutputFormat) {
    print_formatted(attachments, format, |attachments| {
        if attachments.is_empty() {
            println!("No attachments.");
            return;
        }
        for a in attachments {
            let obsolete = if a.is_obsolete { " [OBSOLETE]" } else { "" };
            let private = if a.is_private { " [PRIVATE]" } else { "" };
            println!(
                "{} #{} - {}{}{}",
                "Attachment".bold(),
                a.id,
                a.summary.bold(),
                obsolete.red(),
                private.red(),
            );
            println!(
                "  File:     {} ({}, {} bytes)",
                a.file_name, a.content_type, a.size
            );
            println!("  Creator:  {}", a.creator.as_deref().unwrap_or("-"));
            println!("  Created:  {}", a.creation_time.as_deref().unwrap_or("-"));
            println!();
        }
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use crate::types::Attachment;

    fn make_attachment(id: u64, summary: &str) -> Attachment {
        Attachment {
            id,
            bug_id: 42,
            file_name: format!("file_{id}.patch"),
            summary: summary.into(),
            content_type: "text/plain".into(),
            creator: Some("author@example.com".into()),
            creation_time: Some("2025-03-01T09:00:00Z".into()),
            last_change_time: Some("2025-03-02T10:00:00Z".into()),
            size: 1234,
            is_obsolete: false,
            is_private: false,
            data: None,
        }
    }

    #[test]
    fn print_attachments_json_empty() {
        let attachments: Vec<Attachment> = vec![];
        let json = serde_json::to_string_pretty(&attachments).unwrap();
        assert_eq!(json, "[]");
    }

    #[test]
    fn print_attachments_json_one_attachment() {
        let attachments = vec![make_attachment(10, "Fix patch")];
        let json = serde_json::to_string_pretty(&attachments).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["id"], 10);
        assert_eq!(parsed[0]["summary"], "Fix patch");
        assert_eq!(parsed[0]["file_name"], "file_10.patch");
        assert_eq!(parsed[0]["content_type"], "text/plain");
        assert_eq!(parsed[0]["size"], 1234);
    }

    #[test]
    fn print_attachments_json_obsolete_and_private() {
        let mut att = make_attachment(11, "Old patch");
        att.is_obsolete = true;
        att.is_private = true;
        let json = serde_json::to_string(&att).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["is_obsolete"], true);
        assert_eq!(parsed["is_private"], true);
    }
}
