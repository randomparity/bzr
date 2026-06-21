use std::io::Write;

use colored::{ColoredString, Colorize};
use serde::Serialize;
use tabled::builder::Builder;

use crate::types::{Flag, OutputFormat};

// ── Formatting primitives ───────────────────────────────────────────

/// Join a bug or attachment's flags into the concise comma-separated
/// `name<status>[(requestee)]` inline form used by table columns and detail
/// rows on both resources.
pub(super) fn render_flags_inline(flags: &[Flag]) -> String {
    flags
        .iter()
        .map(Flag::render_inline)
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn write_json<W: Write + ?Sized>(value: &(impl Serialize + ?Sized), out: &mut W) {
    let _ = writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(value).expect("serializable to JSON")
    );
}

/// Newline-delimited JSON. A top-level array emits one compact element per line
/// (the streaming shape agents iterate); any other value emits as one compact
/// line. An empty array emits nothing — zero records is zero lines.
pub(crate) fn write_ndjson<W: Write + ?Sized>(value: &(impl Serialize + ?Sized), out: &mut W) {
    let value = serde_json::to_value(value).expect("serializable to JSON");
    match value {
        serde_json::Value::Array(items) => {
            for item in &items {
                let _ = writeln!(out, "{item}");
            }
        }
        other => {
            let _ = writeln!(out, "{other}");
        }
    }
}

/// Render a value in whichever JSON family `format` selects: pretty-printed for
/// [`OutputFormat::Json`], newline-delimited for [`OutputFormat::Ndjson`]. Used
/// by the bespoke `match format` sites whose `Table` arm renders something
/// other than a serialized value; [`write_formatted`] is the simpler entry
/// point when the table form is also a function of the same value.
pub(crate) fn write_json_family<W: Write + ?Sized>(
    value: &(impl Serialize + ?Sized),
    format: OutputFormat,
    out: &mut W,
) {
    match format {
        OutputFormat::Ndjson => write_ndjson(value, out),
        // `Table` is unreachable — callers gate this on `is_json_family()` — but
        // folding it into the pretty-JSON arm keeps the match exhaustive.
        OutputFormat::Json | OutputFormat::Table => write_json(value, out),
    }
}

pub(super) fn write_formatted<T, W>(
    value: &T,
    format: OutputFormat,
    out: &mut W,
    table_fn: impl FnOnce(&T, &mut W),
) where
    T: Serialize + ?Sized,
    W: Write + ?Sized,
{
    match format {
        OutputFormat::Json => write_json(value, out),
        OutputFormat::Ndjson => write_ndjson(value, out),
        OutputFormat::Table => table_fn(value, out),
    }
}

pub(super) fn write_table_records<W: Write + ?Sized>(
    headers: &[&str],
    rows: impl IntoIterator<Item = Vec<String>>,
    out: &mut W,
) {
    let mut builder = Builder::default();
    builder.push_record(headers.iter().copied());
    for row in rows {
        builder.push_record(row);
    }
    let _ = writeln!(out, "{}", builder.build());
}

#[derive(Clone, Copy)]
pub(super) struct TableSpec<'a> {
    pub empty_msg: &'a str,
    pub headers: &'a [&'a str],
}

/// Render a slice as JSON or a `tabled` table, printing `empty_msg` instead of
/// an empty table when there are no items. `to_record` maps each item to one
/// display row.
pub(super) fn write_table_or_empty<T, W>(
    items: &[T],
    format: OutputFormat,
    out: &mut W,
    table: TableSpec<'_>,
    to_record: impl Fn(&T) -> Vec<String>,
) where
    T: Serialize,
    W: Write + ?Sized,
{
    write_formatted(items, format, out, |items, out| {
        if items.is_empty() {
            let _ = writeln!(out, "{}", table.empty_msg);
            return;
        }
        write_table_records(table.headers, items.iter().map(to_record), out);
    });
}

// ── Detail-field helpers ────────────────────────────────────────────
// Shared formatting for bug/resource detail views. All use consistent
// 12-char label alignment and render absent values as "-".

pub(super) fn write_field<W: Write + ?Sized>(out: &mut W, label: &str, value: &str) {
    let _ = writeln!(out, "  {label:<12}  {value}");
}

pub(super) fn write_optional_field<W: Write + ?Sized>(
    out: &mut W,
    label: &str,
    value: Option<&str>,
) {
    let _ = writeln!(out, "  {label:<12}  {}", value.unwrap_or("-"));
}

pub(super) fn write_list_field<W: Write + ?Sized>(out: &mut W, label: &str, items: &[String]) {
    if !items.is_empty() {
        let _ = writeln!(out, "  {label:<12}  {}", items.join(", "));
    }
}

pub(super) fn write_bool_field<W: Write + ?Sized>(out: &mut W, label: &str, value: bool) {
    let _ = writeln!(out, "  {label:<12}  {}", yes_no(value));
}

// ── Section divider ─────────────────────────────────────────────────

/// Width of the horizontal divider used between detail blocks in
/// `bug history`, `bug view` (multi-ID), and similar resource-detail
/// outputs. Box-drawing horizontal bar (`─`, U+2500) repeated this
/// many times.
pub(super) const DIVIDER_WIDTH: usize = 60;

/// Write a horizontal section divider followed by a newline.
pub(crate) fn write_divider<W: Write + ?Sized>(out: &mut W) {
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

/// Column width for bug summaries in table output. Wider than
/// [`DESCRIPTION_TRUNCATE_WIDTH`] because summaries are the primary
/// identifying text in `bug list`/`bug search` rows.
pub(super) const SUMMARY_TRUNCATE_WIDTH: usize = 72;

/// Column width for product/classification descriptions in table output.
pub(super) const DESCRIPTION_TRUNCATE_WIDTH: usize = 60;

pub(super) fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() > max_chars {
        let truncated: String = s.chars().take(max_chars.saturating_sub(3)).collect();
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

pub(super) fn colorize_status(status: &str) -> ColoredString {
    match status.to_uppercase().as_str() {
        "NEW" | "UNCONFIRMED" => status.green(),
        "ASSIGNED" | "IN_PROGRESS" => status.yellow(),
        "RESOLVED" | "VERIFIED" | "CLOSED" => status.red(),
        _ => status.normal(),
    }
}

pub(super) fn mask_api_key(key: &str) -> String {
    if key.chars().count() > 8 {
        let prefix: String = key.chars().take(8).collect();
        format!("{prefix}...")
    } else {
        "***".into()
    }
}

#[cfg(test)]
#[path = "formatting_tests.rs"]
mod tests;
