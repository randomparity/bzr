use std::io::{self, Write};

use colored::Colorize;
use serde::Serialize;

use crate::types::OutputFormat;

// ── Formatting primitives ───────────────────────────────────────────

pub(super) fn print_json(value: &(impl Serialize + ?Sized)) {
    writeln!(
        io::stdout(),
        "{}",
        serde_json::to_string_pretty(value).expect("serializable to JSON")
    )
    .expect("write to output");
}

pub(super) fn print_formatted<T: Serialize + ?Sized>(
    value: &T,
    format: OutputFormat,
    table_fn: impl FnOnce(&T),
) {
    match format {
        OutputFormat::Json => print_json(value),
        OutputFormat::Table => table_fn(value),
    }
}

// ── Detail-field helpers ────────────────────────────────────────────
// Shared formatting for bug/resource detail views. All use consistent
// 12-char label alignment and render absent values as "-".

pub(super) fn print_field(label: &str, value: &str) {
    writeln!(io::stdout(), "  {label:<12}  {value}").expect("write to output");
}

pub(super) fn print_optional_field(label: &str, value: Option<&str>) {
    writeln!(io::stdout(), "  {label:<12}  {}", value.unwrap_or("-")).expect("write to output");
}

pub(super) fn print_list_field(label: &str, items: &[String]) {
    if !items.is_empty() {
        writeln!(io::stdout(), "  {label:<12}  {}", items.join(", ")).expect("write to output");
    }
}

pub(super) fn print_id_list_field(label: &str, ids: &[u64]) {
    if !ids.is_empty() {
        writeln!(io::stdout(), "  {label:<12}  {}", format_id_list(ids)).expect("write to output");
    }
}

pub(super) fn print_bool_field(label: &str, value: bool) {
    writeln!(io::stdout(), "  {label:<12}  {}", yes_no(value)).expect("write to output");
}

pub(super) fn format_id_list(ids: &[u64]) -> String {
    ids.iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn yes_no(value: bool) -> &'static str {
    if value {
        "Yes"
    } else {
        "No"
    }
}

/// Three-valued yes/no for `Option<bool>` — returns "Yes", "No", or "-".
pub(super) fn opt_yes_no(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "Yes",
        Some(false) => "No",
        None => "-",
    }
}

// ── Text helpers ────────────────────────────────────────────────────

pub(super) fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        let truncated: String = s.chars().take(max_chars - 3).collect();
        format!("{truncated}...")
    } else {
        s.to_string()
    }
}

pub(super) fn shorten_email(email: &str) -> String {
    if let Some(at) = email.find('@') {
        email[..at].to_string()
    } else {
        email.to_string()
    }
}

pub(super) fn colorize_status(status: &str) -> String {
    match status.to_uppercase().as_str() {
        "NEW" | "UNCONFIRMED" => status.green().to_string(),
        "ASSIGNED" | "IN_PROGRESS" => status.yellow().to_string(),
        "RESOLVED" | "VERIFIED" | "CLOSED" => status.red().to_string(),
        _ => status.to_string(),
    }
}

pub(super) fn mask_api_key(key: &str) -> String {
    if key.len() > 8 {
        format!("{}...", &key[..8])
    } else {
        "***".into()
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::types::OutputFormat;

    // ── truncate ─────────────────────────────────────────────────────

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact_length_unchanged() {
        assert_eq!(truncate("abcde", 5), "abcde");
    }

    #[test]
    fn truncate_long_string_adds_ellipsis() {
        let result = truncate("abcdefghij", 7);
        assert_eq!(result, "abcd...");
        assert_eq!(result.len(), 7);
    }

    #[test]
    fn truncate_unicode_counts_chars_not_bytes() {
        let input = "\u{1f600}\u{1f601}\u{1f602}\u{1f603}";
        let result = truncate(input, 4);
        assert_eq!(result, input);
        let result = truncate(input, 3);
        assert!(result.ends_with("..."));
    }

    // ── colorize_status ──────────────────────────────────────────────

    #[test]
    fn colorize_status_new_is_green() {
        let result = colorize_status("NEW");
        assert!(result.contains("NEW"));
    }

    #[test]
    fn colorize_status_assigned_is_yellow() {
        let result = colorize_status("ASSIGNED");
        assert!(result.contains("ASSIGNED"));
    }

    #[test]
    fn colorize_status_resolved_is_red() {
        let result = colorize_status("RESOLVED");
        assert!(result.contains("RESOLVED"));
    }

    #[test]
    fn colorize_status_unknown_passes_through() {
        let result = colorize_status("CUSTOM");
        assert!(result.contains("CUSTOM"));
    }

    #[test]
    fn colorize_status_case_insensitive() {
        let result = colorize_status("new");
        assert!(result.contains("new"));
    }

    // ── shorten_email ────────────────────────────────────────────────

    #[test]
    fn shorten_email_strips_domain() {
        assert_eq!(shorten_email("alice@example.com"), "alice");
    }

    #[test]
    fn shorten_email_no_at_unchanged() {
        assert_eq!(shorten_email("alice"), "alice");
    }

    #[test]
    fn shorten_email_empty_string() {
        assert_eq!(shorten_email(""), "");
    }

    // ── format_id_list ───────────────────────────────────────────────

    #[test]
    fn format_id_list_empty() {
        assert_eq!(format_id_list(&[]), "");
    }

    #[test]
    fn format_id_list_single() {
        assert_eq!(format_id_list(&[42]), "42");
    }

    #[test]
    fn format_id_list_multiple() {
        assert_eq!(format_id_list(&[1, 2, 3]), "1, 2, 3");
    }

    // ── OutputFormat parsing ─────────────────────────────────────────

    #[test]
    fn output_format_from_str() {
        assert_eq!(
            "table".parse::<OutputFormat>().unwrap(),
            OutputFormat::Table
        );
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert!("JSON".parse::<OutputFormat>().is_err());
        assert!("Table".parse::<OutputFormat>().is_err());
        assert!("xml".parse::<OutputFormat>().is_err());
        let err = "XML".parse::<OutputFormat>().unwrap_err();
        assert!(err.contains("expected 'table' or 'json'"));
    }

    #[test]
    fn output_format_default_is_table() {
        assert_eq!(OutputFormat::default(), OutputFormat::Table);
    }

    // ── print_result ─────────────────────────────────────────────────

    #[test]
    fn print_result_json_serializes_value() {
        let value = serde_json::json!({"id": 42});
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, r#"{"id":42}"#);
    }

    // ── mask_api_key tests ──────────────────────────────────────────

    #[test]
    fn mask_api_key_long_key_shows_prefix() {
        assert_eq!(mask_api_key("abcdefghijklmnop"), "abcdefgh...");
    }

    #[test]
    fn mask_api_key_short_key_fully_masked() {
        assert_eq!(mask_api_key("short"), "***");
    }

    #[test]
    fn mask_api_key_exactly_8_chars_fully_masked() {
        assert_eq!(mask_api_key("12345678"), "***");
    }

    #[test]
    fn mask_api_key_empty_string_fully_masked() {
        assert_eq!(mask_api_key(""), "***");
    }
}
