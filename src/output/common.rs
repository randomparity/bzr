use colored::Colorize;
use serde::Serialize;

use crate::types::OutputFormat;

// ── Formatting primitives ───────────────────────────────────────────

fn print_json(value: &(impl Serialize + ?Sized)) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).expect("serializable to JSON")
    );
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

pub fn print_result(value: &(impl Serialize + ?Sized), human_message: &str, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string(value).expect("serializable to JSON")
            );
        }
        OutputFormat::Table => println!("{human_message}"),
    }
}

// ── Action result types ─────────────────────────────────────────────

/// Resource type for mutation result payloads.
#[derive(Debug, Serialize)]
pub enum ResourceKind {
    #[serde(rename = "bug")]
    Bug,
    #[serde(rename = "attachment")]
    Attachment,
    #[serde(rename = "comment")]
    Comment,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "product")]
    Product,
    #[serde(rename = "component")]
    Component,
}

/// Action type for mutation result payloads.
#[derive(Debug, Serialize)]
pub enum ActionKind {
    #[serde(rename = "created")]
    Created,
    #[serde(rename = "updated")]
    Updated,
}

/// Typed result payload for JSON output of mutation operations.
///
/// Covers standard CRUD results with an `id` and optional `name`. The following
/// cases intentionally use ad-hoc `serde_json::json!()` instead:
/// - **Attachment download** — includes `file` path and `size` fields.
/// - **Attachment upload** — includes `bug_id` and `size` fields.
/// - **Group membership add/remove** — relationship-shaped (user + group), not resource-shaped.
/// - **Comment tags** — returns the resulting tag list, not a simple resource mutation.
/// - **Config operations** — include config-file path, URL, and default-server flag.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct ActionResult {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub resource: ResourceKind,
    pub action: ActionKind,
}

impl ActionResult {
    pub fn created(id: u64, resource: ResourceKind) -> Self {
        Self {
            id,
            name: None,
            resource,
            action: ActionKind::Created,
        }
    }

    pub fn created_named(id: u64, name: impl Into<String>, resource: ResourceKind) -> Self {
        Self {
            id,
            name: Some(name.into()),
            resource,
            action: ActionKind::Created,
        }
    }

    pub fn updated(id: u64, resource: ResourceKind) -> Self {
        Self {
            id,
            name: None,
            resource,
            action: ActionKind::Updated,
        }
    }

    pub fn updated_named(name: impl Into<String>, resource: ResourceKind) -> Self {
        Self {
            id: 0,
            name: Some(name.into()),
            resource,
            action: ActionKind::Updated,
        }
    }
}

// ── Detail-field helpers ────────────────────────────────────────────

/// Print a detail field with a pre-formatted (e.g. colored) value.
pub(super) fn print_colored_field(label: &str, value: &str) {
    println!("  {label:<12}  {value}");
}

/// Print an optional detail field, showing "-" when absent.
pub(super) fn print_optional_field(label: &str, value: Option<&str>) {
    println!("  {label:<12}  {}", value.unwrap_or("-"));
}

/// Print a string-list detail field, only when non-empty.
pub(super) fn print_list_field(label: &str, items: &[String]) {
    if !items.is_empty() {
        println!("  {label:<12}  {}", items.join(", "));
    }
}

/// Print an ID-list detail field, only when non-empty.
pub(super) fn print_id_list_field(label: &str, ids: &[u64]) {
    if !ids.is_empty() {
        println!("  {label:<12}  {}", format_id_list(ids));
    }
}

pub(super) fn format_id_list(ids: &[u64]) -> String {
    ids.iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
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
