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

pub(super) fn write_field(out: &mut impl Write, label: &str, value: &str) {
    let _ = writeln!(out, "  {label:<12}  {value}");
}

pub(super) fn write_optional_field(out: &mut impl Write, label: &str, value: Option<&str>) {
    let _ = writeln!(out, "  {label:<12}  {}", value.unwrap_or("-"));
}

pub(super) fn write_list_field(out: &mut impl Write, label: &str, items: &[String]) {
    if !items.is_empty() {
        let _ = writeln!(out, "  {label:<12}  {}", items.join(", "));
    }
}

pub(super) fn print_field(label: &str, value: &str) {
    write_field(&mut io::stdout(), label, value);
}

pub(super) fn print_optional_field(label: &str, value: Option<&str>) {
    write_optional_field(&mut io::stdout(), label, value);
}

pub(super) fn print_list_field(label: &str, items: &[String]) {
    write_list_field(&mut io::stdout(), label, items);
}

pub(super) fn print_bool_field(label: &str, value: bool) {
    writeln!(io::stdout(), "  {label:<12}  {}", yes_no(value)).expect("write to output");
}

// ── Section divider ─────────────────────────────────────────────────

/// Width of the horizontal divider used between detail blocks in
/// `bug history`, `bug view` (multi-ID), and similar resource-detail
/// outputs. Box-drawing horizontal bar (`─`, U+2500) repeated this
/// many times.
pub(super) const DIVIDER_WIDTH: usize = 60;

/// Write a horizontal section divider followed by a newline.
pub(crate) fn write_divider(out: &mut impl Write) {
    let _ = writeln!(out, "{}", "─".repeat(DIVIDER_WIDTH));
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
#[path = "formatting_tests.rs"]
mod tests;
