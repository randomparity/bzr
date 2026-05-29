use std::io::Write;

use colored::Colorize;
use tabled::builder::Builder;

use crate::output::formatting::{
    colorize_status, shorten_email, truncate, write_divider, write_field, write_formatted,
    write_json, write_list_field, write_optional_field, SUMMARY_TRUNCATE_WIDTH,
};
use crate::types::{Bug, HistoryEntry, OutputFormat};

/// Which fields the caller asked to include / exclude, as the raw
/// comma-separated `--fields` / `--exclude-fields` values. `Default`
/// (both `None`) means "use the default column set".
#[derive(Debug, Clone, Copy, Default)]
pub struct ColumnSpec<'a> {
    pub include: Option<&'a str>,
    pub exclude: Option<&'a str>,
}

/// A selectable column in bug table output: the tokens that map to it,
/// its header, and how to render one bug's cell.
struct BugColumn {
    /// Accepted field tokens (lowercase) that resolve to this column. By
    /// convention `aliases[0]` is the canonical Bugzilla field name used for
    /// the server's `include_fields`/`exclude_fields` payload; the remaining
    /// entries are accepted synonyms for column selection only.
    aliases: &'static [&'static str],
    header: &'static str,
    render: fn(&Bug) -> String,
}

impl BugColumn {
    /// The canonical Bugzilla field name (first alias), used when building
    /// the server's `include_fields`/`exclude_fields` payload.
    fn canonical(&self) -> &'static str {
        self.aliases[0]
    }
}

/// Columns shown when `--fields` is not supplied. Order and headers match
/// the historical fixed table.
const DEFAULT_COLUMNS: &[&str] = &["id", "status", "priority", "assignee", "summary"];

/// The full set of fields renderable as table columns. Tokens are matched
/// case-insensitively against `aliases`. Fields absent here (e.g. custom
/// `cf_*` fields) have no table representation.
const COLUMNS: &[BugColumn] = &[
    BugColumn {
        aliases: &["id"],
        header: "ID",
        render: |b| b.id.to_string(),
    },
    BugColumn {
        aliases: &["status"],
        header: "STATUS",
        render: |b| b.status.clone(),
    },
    BugColumn {
        aliases: &["priority"],
        header: "PRIORITY",
        render: |b| b.priority.clone().unwrap_or_default(),
    },
    BugColumn {
        aliases: &["assigned_to", "assignee"],
        header: "ASSIGNEE",
        render: |b| shorten_email(b.assigned_to.as_deref().unwrap_or("")),
    },
    BugColumn {
        aliases: &["summary"],
        header: "SUMMARY",
        render: |b| truncate(&b.summary, SUMMARY_TRUNCATE_WIDTH),
    },
    BugColumn {
        aliases: &["severity"],
        header: "SEVERITY",
        render: |b| b.severity.clone().unwrap_or_default(),
    },
    BugColumn {
        aliases: &["product"],
        header: "PRODUCT",
        render: |b| b.product.clone().unwrap_or_default(),
    },
    BugColumn {
        aliases: &["component"],
        header: "COMPONENT",
        render: |b| b.component.clone().unwrap_or_default(),
    },
    BugColumn {
        aliases: &["resolution"],
        header: "RESOLUTION",
        render: |b| b.resolution.clone().unwrap_or_default(),
    },
    BugColumn {
        aliases: &["version"],
        header: "VERSION",
        render: |b| b.version.clone().unwrap_or_default(),
    },
    BugColumn {
        aliases: &["creator", "reporter"],
        header: "CREATOR",
        render: |b| b.creator.clone().unwrap_or_default(),
    },
    BugColumn {
        aliases: &["creation_time", "created"],
        header: "CREATED",
        render: |b| b.creation_time.clone().unwrap_or_default(),
    },
    BugColumn {
        aliases: &["last_change_time", "updated"],
        header: "UPDATED",
        render: |b| b.last_change_time.clone().unwrap_or_default(),
    },
    BugColumn {
        aliases: &["url"],
        header: "URL",
        render: |b| b.url.clone().unwrap_or_default(),
    },
    BugColumn {
        aliases: &["whiteboard"],
        header: "WHITEBOARD",
        render: |b| b.whiteboard.clone().unwrap_or_default(),
    },
    BugColumn {
        aliases: &["op_sys"],
        header: "OP_SYS",
        render: |b| b.op_sys.clone().unwrap_or_default(),
    },
    BugColumn {
        aliases: &["rep_platform", "platform"],
        header: "PLATFORM",
        render: |b| b.rep_platform.clone().unwrap_or_default(),
    },
    BugColumn {
        aliases: &["deadline"],
        header: "DEADLINE",
        render: |b| b.deadline.clone().unwrap_or_default(),
    },
    BugColumn {
        aliases: &["keywords"],
        header: "KEYWORDS",
        render: |b| b.keywords.join(", "),
    },
    BugColumn {
        aliases: &["blocks"],
        header: "BLOCKS",
        render: |b| join_ids(&b.blocks),
    },
    BugColumn {
        aliases: &["depends_on"],
        header: "DEPENDS_ON",
        render: |b| join_ids(&b.depends_on),
    },
    BugColumn {
        aliases: &["cc"],
        header: "CC",
        render: |b| b.cc.join(", "),
    },
    BugColumn {
        aliases: &["dupe_of"],
        header: "DUPE_OF",
        render: |b| b.dupe_of.map(|id| id.to_string()).unwrap_or_default(),
    },
];

fn join_ids(ids: &[u64]) -> String {
    ids.iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Resolve a single field token to its column, case-insensitively.
fn resolve_bug_column(token: &str) -> Option<&'static BugColumn> {
    let token = token.trim().to_ascii_lowercase();
    COLUMNS.iter().find(|c| c.aliases.contains(&token.as_str()))
}

/// Translate a comma-separated field list (which may use column aliases such
/// as `assignee` or `updated`) into canonical Bugzilla field names for the
/// server's `include_fields` / `exclude_fields` parameters. Unknown tokens
/// (e.g. custom `cf_*` fields) pass through unchanged. Empty input or an
/// all-empty list yields `None`.
pub fn canonical_field_list(fields: Option<&str>) -> Option<String> {
    let fields = fields?;
    let mut out: Vec<&str> = Vec::new();
    for token in fields.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        match resolve_bug_column(token) {
            Some(col) => out.push(col.canonical()),
            None => out.push(token),
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out.join(","))
    }
}

fn default_columns() -> Vec<&'static BugColumn> {
    DEFAULT_COLUMNS
        .iter()
        .filter_map(|name| resolve_bug_column(name))
        .collect()
}

/// Split a comma list into (resolved columns, unknown tokens), trimming and
/// skipping blanks. Shared by `resolve_columns` and `validate_table_columns`
/// so the renderer and the pre-flight validator can't drift.
fn partition_include(list: &str) -> (Vec<&'static BugColumn>, Vec<&str>) {
    let mut knowns = Vec::new();
    let mut unknowns = Vec::new();
    for token in list.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        match resolve_bug_column(token) {
            Some(col) => knowns.push(col),
            None => unknowns.push(token),
        }
    }
    (knowns, unknowns)
}

/// Apply `spec.exclude` to `columns` in place, dropping any column whose
/// header matches an excluded token.
fn apply_exclude(columns: &mut Vec<&'static BugColumn>, exclude: Option<&str>) {
    if let Some(list) = exclude {
        let excluded: Vec<&'static BugColumn> =
            list.split(',').filter_map(resolve_bug_column).collect();
        columns.retain(|c| !excluded.iter().any(|e| e.header == c.header));
    }
}

/// Resolve `spec` into the ordered list of columns to render. Unknown
/// include tokens are reported as a warning on `err`. If every requested
/// token is unknown, falls back to the default column set so output stays
/// useful. Infallible by design — the fully-degenerate cases (zero columns)
/// are rejected up front by [`validate_table_columns`].
fn resolve_columns<E: Write + ?Sized>(
    spec: ColumnSpec<'_>,
    err: &mut E,
) -> Vec<&'static BugColumn> {
    let mut columns = match spec.include {
        None => default_columns(),
        Some(list) => {
            let (knowns, unknowns) = partition_include(list);
            if !unknowns.is_empty() {
                let _ = writeln!(
                    err,
                    "warning: ignoring field(s) with no table column: {}",
                    unknowns.join(", ")
                );
            }
            if knowns.is_empty() {
                default_columns()
            } else {
                knowns
            }
        }
    };
    apply_exclude(&mut columns, spec.exclude);
    columns
}

/// Validate that `spec` yields at least one renderable table column. Call
/// ONLY when output is a table, before the network request. Errors (exit 7)
/// when a `--fields` value resolves to zero columns (all tokens unknown), or
/// when `--exclude-fields` removes every column. Partial-unknown (some valid,
/// some not) is allowed and handled as a warning at render time.
pub fn validate_table_columns(spec: ColumnSpec<'_>) -> crate::error::Result<()> {
    let mut columns = match spec.include {
        None => default_columns(),
        Some(list) => {
            let (knowns, unknowns) = partition_include(list);
            if knowns.is_empty() {
                if unknowns.is_empty() {
                    // All-blank like ",," — treat as no selection, not an error.
                    default_columns()
                } else {
                    return Err(crate::error::BzrError::InputValidation(format!(
                        "none of the requested fields can be shown as table columns: {}; \
                         these fields have no table representation",
                        unknowns.join(", ")
                    )));
                }
            } else {
                knowns
            }
        }
    };
    apply_exclude(&mut columns, spec.exclude);
    if columns.is_empty() {
        return Err(crate::error::BzrError::InputValidation(
            "--exclude-fields removed every table column; nothing left to display".into(),
        ));
    }
    Ok(())
}

/// The canonical exclude key set: each `--exclude-fields` token resolved to its
/// canonical Bugzilla field name. Unknown tokens (e.g. custom `cf_*` fields)
/// name no key in the serialized object and are dropped here, matching table
/// mode's [`apply_exclude`].
fn canonical_excludes(exclude: Option<&str>) -> Vec<&'static str> {
    match exclude {
        None => Vec::new(),
        Some(list) => list
            .split(',')
            .filter_map(resolve_bug_column)
            .map(BugColumn::canonical)
            .collect(),
    }
}

/// Project a serialized bug object to honor `spec` (gh-style trimming):
/// a non-blank `include` retains exactly the named canonical keys; `exclude`
/// drops the named canonical keys; neither (or a blank include) leaves the
/// object untouched. Aliases resolve to canonical keys via the same primitives
/// table mode uses. Unknown tokens are inert — they name no key in the object,
/// so they neither add nor remove anything; the pre-network gate warns about
/// them via [`warn_unknown_fields`].
pub fn bug_to_json(bug: &Bug, spec: ColumnSpec<'_>) -> serde_json::Value {
    let mut value = serde_json::to_value(bug).expect("Bug serializes to JSON");
    if let serde_json::Value::Object(map) = &mut value {
        if let Some(include) = canonical_field_list(spec.include) {
            let keep: std::collections::HashSet<&str> = include.split(',').collect();
            map.retain(|k, _| keep.contains(k.as_str()));
        }
        for canonical in canonical_excludes(spec.exclude) {
            map.remove(canonical);
        }
    }
    value
}

/// [`bug_to_json`] over a slice, for the array output paths.
pub fn bugs_to_json(bugs: &[Bug], spec: ColumnSpec<'_>) -> Vec<serde_json::Value> {
    bugs.iter().map(|bug| bug_to_json(bug, spec)).collect()
}

/// Validate that `spec` leaves at least one JSON key to emit, measured against
/// the full bug-field universe (every [`COLUMNS`] canonical key), not table
/// mode's five-column default. Call ONLY when output is JSON, before the
/// network request. Errors (exit 7) when the effective projected key set is
/// empty — i.e. an all-unknown `--fields` value, or an `--exclude-fields` that
/// drops every key. A `--fields` that drops the table defaults but keeps other
/// fields still passes. Partial-unknown (some valid, some not) is allowed and
/// handled as a warning via [`warn_unknown_fields`].
pub fn validate_json_field_selection(spec: ColumnSpec<'_>) -> crate::error::Result<()> {
    let mut keys: std::collections::HashSet<&'static str> = match spec.include {
        Some(list) => {
            let (knowns, _unknowns) = partition_include(list);
            if knowns.is_empty() && list.split(',').all(|t| t.trim().is_empty()) {
                // Blank include like "" / ",," — treat as no selection.
                COLUMNS.iter().map(BugColumn::canonical).collect()
            } else {
                knowns.iter().map(|c| c.canonical()).collect()
            }
        }
        None => COLUMNS.iter().map(BugColumn::canonical).collect(),
    };
    for canonical in canonical_excludes(spec.exclude) {
        keys.remove(canonical);
    }
    if keys.is_empty() {
        return Err(crate::error::BzrError::InputValidation(
            "the field selection leaves no fields to emit; \
             adjust --fields / --exclude-fields"
                .into(),
        ));
    }
    Ok(())
}

/// Warn once on stderr about `--fields` tokens that name no known bug field
/// (e.g. a typo or a custom `cf_*` field). Genericized off "table column" so it
/// reads correctly under both table and JSON output. Only inspects the include
/// list — unknown `--exclude-fields` tokens are inert and silently ignored,
/// matching table mode's [`apply_exclude`].
pub fn warn_unknown_fields<E: Write + ?Sized>(spec: ColumnSpec<'_>, err: &mut E) {
    let Some(list) = spec.include else {
        return;
    };
    let (_knowns, unknowns) = partition_include(list);
    if !unknowns.is_empty() {
        let _ = writeln!(
            err,
            "warning: ignoring unknown field(s): {}",
            unknowns.join(", ")
        );
    }
}

/// Whether a detail-view field should render given `spec`. With no include
/// list, every field shows (minus excludes). Tokens are matched against the
/// column registry so `assignee`/`assigned_to` etc. are equivalent. Fields
/// with no registry entry always show by default.
fn field_selected(spec: ColumnSpec<'_>, field: &str) -> bool {
    let Some(target) = resolve_bug_column(field) else {
        return true;
    };
    let matches = |list: &str| {
        list.split(',')
            .filter_map(resolve_bug_column)
            .any(|c| c.header == target.header)
    };
    let included = spec.include.is_none_or(matches);
    let excluded = spec.exclude.is_some_and(matches);
    included && !excluded
}

pub fn write_bugs<W: Write + ?Sized, E: Write + ?Sized>(
    bugs: &[Bug],
    spec: ColumnSpec<'_>,
    format: OutputFormat,
    out: &mut W,
    err: &mut E,
) {
    match format {
        OutputFormat::Json => write_json(&bugs_to_json(bugs, spec), out),
        OutputFormat::Table => {
            if bugs.is_empty() {
                let _ = writeln!(out, "No bugs found.");
                return;
            }
            let columns = resolve_columns(spec, err);
            let mut builder = Builder::default();
            builder.push_record(columns.iter().map(|c| c.header.to_string()));
            for bug in bugs {
                builder.push_record(columns.iter().map(|c| (c.render)(bug)));
            }
            let _ = writeln!(out, "{}", builder.build());
        }
    }
}

pub fn write_bug_detail<W: Write + ?Sized>(
    bug: &Bug,
    spec: ColumnSpec<'_>,
    format: OutputFormat,
    out: &mut W,
) {
    match format {
        OutputFormat::Json => write_json(&bug_to_json(bug, spec), out),
        OutputFormat::Table => write_bug_detail_table(bug, spec, out),
    }
}

fn write_bug_detail_table(bug: &Bug, spec: ColumnSpec<'_>, out: &mut (impl Write + ?Sized)) {
    if field_selected(spec, "summary") {
        let _ = writeln!(
            out,
            "{} #{}\n{}\n",
            "Bug".bold(),
            bug.id.to_string().bold(),
            bug.summary.bold()
        );
    } else {
        let _ = writeln!(out, "{} #{}\n", "Bug".bold(), bug.id.to_string().bold());
    }
    if field_selected(spec, "status") {
        write_field(out, "Status", &colorize_status(&bug.status));
    }
    if field_selected(spec, "resolution") {
        write_optional_field(out, "Resolution", bug.resolution.as_deref());
    }
    if field_selected(spec, "dupe_of") {
        if let Some(dupe_of) = bug.dupe_of {
            let _ = writeln!(out, "  {:<12}  {dupe_of}", "Duplicate of");
        }
    }
    if field_selected(spec, "product") {
        write_optional_field(out, "Product", bug.product.as_deref());
    }
    if field_selected(spec, "component") {
        write_optional_field(out, "Component", bug.component.as_deref());
    }
    if field_selected(spec, "assigned_to") {
        write_optional_field(out, "Assignee", bug.assigned_to.as_deref());
    }
    if field_selected(spec, "priority") {
        write_optional_field(out, "Priority", bug.priority.as_deref());
    }
    if field_selected(spec, "severity") {
        write_optional_field(out, "Severity", bug.severity.as_deref());
    }
    if field_selected(spec, "creator") {
        write_optional_field(out, "Creator", bug.creator.as_deref());
    }
    if field_selected(spec, "creation_time") {
        write_optional_field(out, "Created", bug.creation_time.as_deref());
    }
    if field_selected(spec, "last_change_time") {
        write_optional_field(out, "Updated", bug.last_change_time.as_deref());
    }
    if field_selected(spec, "keywords") {
        write_list_field(out, "Keywords", &bug.keywords);
    }
    if field_selected(spec, "blocks") {
        write_id_list_field(out, "Blocks", &bug.blocks);
    }
    if field_selected(spec, "depends_on") {
        write_id_list_field(out, "Depends on", &bug.depends_on);
    }
}

fn write_id_list_field(out: &mut (impl Write + ?Sized), label: &str, ids: &[u64]) {
    if !ids.is_empty() {
        let id_str = ids
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "  {label:<12}  {id_str}");
    }
}

pub fn write_history<W: Write + ?Sized>(
    history: &[HistoryEntry],
    format: OutputFormat,
    out: &mut W,
) {
    write_formatted(history, format, out, |history, out| {
        for entry in history {
            let _ = writeln!(
                out,
                "{} by {} ({})",
                "Change".bold(),
                entry.who.cyan(),
                entry.when,
            );
            for change in &entry.changes {
                let attachment_suffix = change
                    .attachment_id
                    .map(|id| format!(" [attachment #{id}]"))
                    .unwrap_or_default();
                let _ = writeln!(out, "  {}{attachment_suffix}:", change.field_name.bold());
                if !change.removed.is_empty() {
                    let _ = writeln!(out, "    - {}", change.removed.red());
                }
                if !change.added.is_empty() {
                    let _ = writeln!(out, "    + {}", change.added.green());
                }
            }
            write_divider(out);
        }
    });
}

/// One row in a multi-ID `bzr bug view` output stream.
///
/// Used by [`write_multi_bug_view`] to interleave successful detail
/// blocks with `UNAVAILABLE` placeholder blocks for inaccessible bugs.
#[non_exhaustive]
#[derive(Debug)]
pub enum MultiBugRow {
    Ok(Box<Bug>),
    Failed { id: String, error: String },
}

/// Render a multi-ID `bzr bug view` result.
///
/// JSON mode is **not** handled here — the caller builds the
/// `{"bugs": [...], "failed": [...]}` wrapper itself (projecting each bug via
/// [`bug_to_json`]). This function only covers table mode: argument-order
/// detail blocks for `Ok`, visually distinct `UNAVAILABLE` placeholder blocks
/// for `Failed`, with a `─`-divider line between every pair of blocks (no
/// trailing divider).
pub fn write_multi_bug_view<W: Write + ?Sized>(
    rows: &[MultiBugRow],
    spec: ColumnSpec<'_>,
    out: &mut W,
) {
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            write_divider(out);
        }
        match row {
            MultiBugRow::Ok(bug) => write_bug_detail_table(bug, spec, out),
            MultiBugRow::Failed { id, error } => write_unavailable_block(id, error, out),
        }
    }
}

fn write_unavailable_block(id: &str, error: &str, out: &mut (impl Write + ?Sized)) {
    let _ = writeln!(
        out,
        "{} #{} — {}",
        "Bug".bold(),
        id.bold(),
        "UNAVAILABLE".red().bold()
    );
    let _ = writeln!(out, "  Error: {error}");
}

#[cfg(test)]
#[path = "bug_tests.rs"]
mod tests;
