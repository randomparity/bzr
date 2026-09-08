use std::io::Write;

use colored::{ColoredString, Colorize};
use serde::Serialize;
use tabled::builder::Builder;
use tabled::settings::{peaker::PriorityMax, Width};
use tabled::Table;

use crate::types::flag::Flag;
use crate::types::output::OutputFormat;

// ── Formatting primitives ───────────────────────────────────────────

#[cfg(test)]
pub(crate) fn disable_color_for_tests() {
    colored::control::set_override(false);
}

#[cfg(not(test))]
pub(crate) fn disable_color_for_tests() {}

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

/// The top-level `--json` envelope: a stable `schema_version` plus the
/// command's `data` payload. Borrows the payload so no clone is needed; field
/// order (`schema_version` first) is preserved by the derive.
#[derive(Serialize)]
struct JsonEnvelope<'a, T: Serialize + ?Sized> {
    schema_version: &'static str,
    data: &'a T,
}

/// Write pretty `--json` output, wrapping the payload in the versioned
/// envelope. This is the single seam for the `schema_version` contract on the
/// success path; `--output ndjson` (see [`write_ndjson`]) stays bare.
pub(super) fn write_json<W: Write + ?Sized>(value: &(impl Serialize + ?Sized), out: &mut W) {
    let envelope = JsonEnvelope {
        schema_version: crate::output::SCHEMA_VERSION,
        data: value,
    };
    let _ = writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(&envelope).expect("serializable to JSON")
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
    disable_color_for_tests();
    match format {
        OutputFormat::Json => write_json(value, out),
        OutputFormat::Ndjson => write_ndjson(value, out),
        OutputFormat::Table => table_fn(value, out),
    }
}

/// Like [`write_formatted`], but trims the JSON-family output to `projection`
/// before writing. The table arm renders via `table_fn` unchanged (projection
/// is a no-op on table output).
pub(super) fn write_formatted_projected<T, W>(
    value: &T,
    format: OutputFormat,
    projection: &crate::validation::fields::FieldProjection,
    out: &mut W,
    table_fn: impl FnOnce(&T, &mut W),
) where
    T: Serialize + ?Sized,
    W: Write + ?Sized,
{
    disable_color_for_tests();
    match format {
        OutputFormat::Json => {
            let mut value = serde_json::to_value(value).expect("serializable to JSON");
            projection.apply(&mut value);
            write_json(&value, out);
        }
        OutputFormat::Ndjson => {
            let mut value = serde_json::to_value(value).expect("serializable to JSON");
            projection.apply(&mut value);
            write_ndjson(&value, out);
        }
        OutputFormat::Table => table_fn(value, out),
    }
}

/// The single table entry point: build, escape, optionally wrap, write.
///
/// Every header and cell passes through [`escape_terminal_controls`], so a
/// writer that renders server data through this function inherits the escaping
/// rather than rediscovering it. This is deliberately the *only* way to emit a
/// table — an entry point taking an already-built [`Table`] could not reach the
/// cells as strings, and leaving one reachable is how the pre-ADR-0065 gap
/// arose.
///
/// Escaping runs after each caller's record closure, so `truncate` still counts
/// the original characters and an escaped cell is pure ASCII whose display width
/// equals its character count — the only form `Width::wrap` measures correctly.
pub(super) fn write_table_records<W: Write + ?Sized>(
    headers: &[&str],
    rows: impl IntoIterator<Item = Vec<String>>,
    width: Option<usize>,
    out: &mut W,
) {
    let mut builder = Builder::default();
    builder.push_record(
        headers
            .iter()
            .map(|header| escape_terminal_controls(header)),
    );
    for row in rows {
        builder.push_record(row.iter().map(|cell| escape_terminal_controls(cell)));
    }
    let mut table: Table = builder.build();
    if let Some(width) = width {
        let minimum_width = 5 * table.count_columns() + 1;
        table.with(Width::wrap(width.max(minimum_width)).priority(PriorityMax::right()));
    }
    let _ = writeln!(out, "{table}");
}

#[derive(Clone, Copy)]
pub(super) struct TableSpec<'a> {
    pub empty_msg: &'a str,
    pub headers: &'a [&'a str],
}

/// Render `items` as a `tabled` table, printing `table.empty_msg` instead of an
/// empty table when there are no items. Table-only (no format branch); shared by
/// the projected resource writers' table closures.
pub(super) fn write_records_or_empty<T, W>(
    items: &[T],
    table: TableSpec<'_>,
    to_record: impl Fn(&T) -> Vec<String>,
    width: Option<usize>,
    out: &mut W,
) where
    W: Write + ?Sized,
{
    if items.is_empty() {
        let _ = writeln!(out, "{}", table.empty_msg);
        return;
    }
    write_table_records(table.headers, items.iter().map(to_record), width, out);
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

/// The Trojan-Source bidirectional formatting characters (CVE-2021-42574): the
/// embedding and override pair, the isolates, and the three implicit marks.
/// Same set as rustc's `text_direction_codepoint_in_literal` lint.
const BIDI_CONTROLS: [char; 12] = [
    '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}',
    '\u{2069}', '\u{200e}', '\u{200f}', '\u{61c}',
];

/// Escape terminal-controlling characters before a server-controlled string is
/// rendered to the terminal.
///
/// Table and detail output go straight to a terminal, so a value the server
/// chose could otherwise let a hostile or compromised Bugzilla manipulate the
/// screen, forge rows, or reorder what a reader sees. Two categories are
/// escaped, via `char::escape_default`:
///
/// - Unicode `Cc` (`char::is_control`), which covers ESC and the C0/C1 ranges.
/// - [`BIDI_CONTROLS`], which reorder rendered text invisibly.
///
/// The rest of `Cf` deliberately passes through: `U+200C`/`U+200D` (ZWNJ, ZWJ)
/// are load-bearing for Persian and Hindi orthography and for emoji sequences,
/// and `U+200B`/`U+FEFF` are invisible but do not reorder. See ADR 0065.
///
/// Applied at three seams — [`write_table_records`], the [`write_field`] family,
/// and `write_status_field` — plus an explicit call at each writer that
/// composes its own line. It does **not** cover `--json`/`--ndjson`:
/// `serde_json` escapes only `"`, `\`, and code points below `0x20`, so bidi
/// passes through the JSON family verbatim. That is a published-schema surface
/// and a deliberate exclusion, not an oversight.
pub(super) fn escape_terminal_controls(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_control() || BIDI_CONTROLS.contains(&character) {
            escaped.extend(character.escape_default());
        } else {
            escaped.push(character);
        }
    }
    escaped
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
