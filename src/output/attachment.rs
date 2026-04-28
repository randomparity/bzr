use std::io::{self, Write as _};

use colored::Colorize;

use super::formatting::{print_field, print_formatted, print_optional_field};
use crate::types::{Attachment, OutputFormat};

pub fn print_attachments(attachments: &[Attachment], format: OutputFormat) {
    print_formatted(attachments, format, |attachments| {
        if attachments.is_empty() {
            let _ = writeln!(io::stdout(), "No attachments.");
            return;
        }
        for a in attachments {
            let obsolete = if a.is_obsolete { " [OBSOLETE]" } else { "" };
            let private = if a.is_private { " [PRIVATE]" } else { "" };
            let _ = writeln!(
                io::stdout(),
                "{} #{} - {}{}{}",
                "Attachment".bold(),
                a.id,
                a.summary.bold(),
                obsolete.red(),
                private.red(),
            );
            print_field(
                "File",
                &format!("{} ({}, {} bytes)", a.file_name, a.content_type, a.size),
            );
            print_optional_field("Creator", a.creator.as_deref());
            print_optional_field("Created", a.creation_time.as_deref());
            let _ = writeln!(io::stdout());
        }
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::print_attachments;
    use crate::types::{Attachment, OutputFormat};

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
    fn attachment_text_format_fields() {
        let att = make_attachment(10, "Fix patch");
        assert_eq!(att.file_name, "file_10.patch");
        assert_eq!(att.content_type, "text/plain");
        assert_eq!(att.size, 1234);
        assert_eq!(att.creator.as_deref(), Some("author@example.com"));
        assert!(!att.is_obsolete);
        assert!(!att.is_private);
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

    // ── capture_stdout-based formatter tests ─────────────────────────

    #[cfg(unix)]
    #[tokio::test]
    async fn print_attachments_table_empty_says_no_attachments() {
        let _lock = crate::ENV_LOCK.lock().await;
        let ((), output) = crate::test_helpers::capture_stdout(async {
            print_attachments(&[], OutputFormat::Table);
        })
        .await;
        assert!(output.contains("No attachments."));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn print_attachments_json_empty_renders_empty_array() {
        let _lock = crate::ENV_LOCK.lock().await;
        let ((), output) = crate::test_helpers::capture_stdout(async {
            print_attachments(&[], OutputFormat::Json);
        })
        .await;
        let parsed = crate::test_helpers::extract_json(&output);
        assert!(parsed.is_array());
        assert_eq!(parsed.as_array().unwrap().len(), 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn print_attachments_table_renders_all_fields() {
        let _lock = crate::ENV_LOCK.lock().await;
        let mut att = make_attachment(7, "Helpful patch");
        att.is_obsolete = true;
        att.is_private = true;
        let attachments = vec![att];
        let ((), output) = crate::test_helpers::capture_stdout(async {
            print_attachments(&attachments, OutputFormat::Table);
        })
        .await;
        assert!(output.contains("Attachment"));
        assert!(output.contains("#7"));
        assert!(output.contains("Helpful patch"));
        assert!(output.contains("[OBSOLETE]"));
        assert!(output.contains("[PRIVATE]"));
        assert!(output.contains("file_7.patch"));
        assert!(output.contains("text/plain"));
        assert!(output.contains("1234 bytes"));
        assert!(output.contains("Creator"));
        assert!(output.contains("author@example.com"));
        assert!(output.contains("Created"));
        assert!(output.contains("2025-03-01T09:00:00Z"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn print_attachments_table_missing_optional_fields_render_dash() {
        let _lock = crate::ENV_LOCK.lock().await;
        let attachments = vec![Attachment {
            id: 8,
            bug_id: 42,
            file_name: "patché.txt".into(),
            summary: "Unicode summary — é".into(),
            content_type: "text/plain".into(),
            creator: None,
            creation_time: None,
            last_change_time: None,
            size: 0,
            is_obsolete: false,
            is_private: false,
            data: None,
        }];
        let ((), output) = crate::test_helpers::capture_stdout(async {
            print_attachments(&attachments, OutputFormat::Table);
        })
        .await;
        assert!(output.contains("Unicode summary — é"));
        assert!(output.contains("patché.txt"));
        // Missing creator/created render as "-"
        assert!(output.contains("Creator"));
        assert!(output.contains("Created"));
        assert!(output.contains('-'));
        // Neither flag is set
        assert!(!output.contains("[OBSOLETE]"));
        assert!(!output.contains("[PRIVATE]"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn print_attachments_json_one_via_print() {
        let _lock = crate::ENV_LOCK.lock().await;
        let attachments = vec![make_attachment(99, "Json patch")];
        let ((), output) = crate::test_helpers::capture_stdout(async {
            print_attachments(&attachments, OutputFormat::Json);
        })
        .await;
        let parsed = crate::test_helpers::extract_json(&output);
        assert_eq!(parsed[0]["id"], 99);
        assert_eq!(parsed[0]["summary"], "Json patch");
        assert_eq!(parsed[0]["file_name"], "file_99.patch");
    }
}
