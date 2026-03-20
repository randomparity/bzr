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
    #[serde(rename = "server")]
    Server,
}

/// Action type for mutation result payloads.
#[derive(Debug, Serialize)]
pub enum ActionKind {
    #[serde(rename = "created")]
    Created,
    #[serde(rename = "updated")]
    Updated,
    #[serde(rename = "added")]
    Added,
    #[serde(rename = "removed")]
    Removed,
    #[serde(rename = "downloaded")]
    Downloaded,
}

/// Typed result payload for relationship mutations (e.g. group membership).
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct MembershipResult {
    pub user: String,
    pub group: String,
    pub resource: ResourceKind,
    pub action: ActionKind,
}

impl MembershipResult {
    pub fn added(user: impl Into<String>, group: impl Into<String>) -> Self {
        Self {
            user: user.into(),
            group: group.into(),
            resource: ResourceKind::Group,
            action: ActionKind::Added,
        }
    }

    pub fn removed(user: impl Into<String>, group: impl Into<String>) -> Self {
        Self {
            user: user.into(),
            group: group.into(),
            resource: ResourceKind::Group,
            action: ActionKind::Removed,
        }
    }
}

/// Typed result payload for attachment download operations.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct DownloadResult {
    pub id: u64,
    pub file: String,
    pub size: usize,
    pub resource: ResourceKind,
    pub action: ActionKind,
}

impl DownloadResult {
    pub fn new(id: u64, file: impl Into<String>, size: usize) -> Self {
        Self {
            id,
            file: file.into(),
            size,
            resource: ResourceKind::Attachment,
            action: ActionKind::Downloaded,
        }
    }
}

/// Typed result payload for attachment upload operations.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct UploadResult {
    pub id: u64,
    pub bug_id: u64,
    pub size: usize,
    pub resource: ResourceKind,
    pub action: ActionKind,
}

impl UploadResult {
    pub fn new(id: u64, bug_id: u64, size: usize) -> Self {
        Self {
            id,
            bug_id,
            size,
            resource: ResourceKind::Attachment,
            action: ActionKind::Created,
        }
    }
}

/// Typed result payload for comment tag operations.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct TagResult {
    pub comment_id: u64,
    pub tags: Vec<String>,
    pub resource: ResourceKind,
    pub action: ActionKind,
}

impl TagResult {
    pub fn updated(comment_id: u64, tags: Vec<String>) -> Self {
        Self {
            comment_id,
            tags,
            resource: ResourceKind::Comment,
            action: ActionKind::Updated,
        }
    }
}

/// Typed result payload for config operations.
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct ConfigResult {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_default: Option<bool>,
    pub config_file: String,
    pub resource: ResourceKind,
    pub action: ActionKind,
}

impl ConfigResult {
    pub fn configured(
        name: impl Into<String>,
        url: impl Into<String>,
        is_default: bool,
        config_file: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            url: Some(url.into()),
            is_default: Some(is_default),
            config_file: config_file.into(),
            resource: ResourceKind::Server,
            action: ActionKind::Created,
        }
    }

    pub fn default_set(name: impl Into<String>, config_file: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: None,
            is_default: None,
            config_file: config_file.into(),
            resource: ResourceKind::Server,
            action: ActionKind::Updated,
        }
    }
}

/// Typed result payload for list-shaped search results (e.g. tag search).
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct SearchResult {
    pub items: Vec<String>,
}

impl SearchResult {
    pub fn new(items: Vec<String>) -> Self {
        Self { items }
    }
}

/// Typed result payload for JSON output of mutation operations.
///
/// Covers standard CRUD results with an `id` and optional `name`.
/// Relationship mutations use [`MembershipResult`], attachment I/O uses
/// [`DownloadResult`]/[`UploadResult`], tag operations use [`TagResult`],
/// config operations use [`ConfigResult`], and search results use
/// [`SearchResult`].
#[derive(Debug, Serialize)]
#[non_exhaustive]
pub struct ActionResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub resource: ResourceKind,
    pub action: ActionKind,
}

impl ActionResult {
    pub fn created(id: u64, resource: ResourceKind) -> Self {
        Self {
            id: Some(id),
            name: None,
            resource,
            action: ActionKind::Created,
        }
    }

    pub fn created_named(id: u64, name: impl Into<String>, resource: ResourceKind) -> Self {
        Self {
            id: Some(id),
            name: Some(name.into()),
            resource,
            action: ActionKind::Created,
        }
    }

    pub fn updated(id: u64, resource: ResourceKind) -> Self {
        Self {
            id: Some(id),
            name: None,
            resource,
            action: ActionKind::Updated,
        }
    }

    pub fn updated_named(name: impl Into<String>, resource: ResourceKind) -> Self {
        Self {
            id: None,
            name: Some(name.into()),
            resource,
            action: ActionKind::Updated,
        }
    }
}

// ── Detail-field helpers ────────────────────────────────────────────
// Shared formatting for bug/resource detail views. All use consistent
// 12-char label alignment and render absent values as "-".

pub(super) fn print_colored_field(label: &str, value: &str) {
    println!("  {label:<12}  {value}");
}

pub(super) fn print_optional_field(label: &str, value: Option<&str>) {
    println!("  {label:<12}  {}", value.unwrap_or("-"));
}

pub(super) fn print_list_field(label: &str, items: &[String]) {
    if !items.is_empty() {
        println!("  {label:<12}  {}", items.join(", "));
    }
}

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

pub(super) fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

/// Three-valued yes/no for `Option<bool>` — returns "Yes", "No", or "-".
pub(super) fn opt_yes_no(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "Yes",
        Some(false) => "No",
        None => "-",
    }
}

pub(super) fn print_bool_field(label: &str, value: bool) {
    println!("  {label:<12}  {}", yes_no(value));
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
