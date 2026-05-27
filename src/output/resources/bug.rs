use std::io::Write;

use colored::Colorize;
use tabled::builder::Builder;

use crate::output::formatting::{
    colorize_status, shorten_email, truncate, write_divider, write_field, write_formatted,
    write_list_field, write_optional_field,
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
    /// Accepted field tokens (lowercase) that resolve to this column.
    aliases: &'static [&'static str],
    header: &'static str,
    render: fn(&Bug) -> String,
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
        aliases: &["assignee", "assigned_to"],
        header: "ASSIGNEE",
        render: |b| shorten_email(b.assigned_to.as_deref().unwrap_or("")),
    },
    BugColumn {
        aliases: &["summary"],
        header: "SUMMARY",
        render: |b| truncate(&b.summary, 72),
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
        aliases: &["platform", "rep_platform"],
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

fn default_columns() -> Vec<&'static BugColumn> {
    DEFAULT_COLUMNS
        .iter()
        .filter_map(|name| resolve_bug_column(name))
        .collect()
}

/// Resolve `spec` into the ordered list of columns to render. Unknown
/// include tokens are collected and reported on `err`. If every requested
/// token is unknown, falls back to the default column set so output stays
/// useful.
fn resolve_columns<E: Write + ?Sized>(
    spec: ColumnSpec<'_>,
    err: &mut E,
) -> Vec<&'static BugColumn> {
    let mut columns = match spec.include {
        None => default_columns(),
        Some(list) => {
            let mut cols = Vec::new();
            let mut unknown = Vec::new();
            for token in list.split(',') {
                let token = token.trim();
                if token.is_empty() {
                    continue;
                }
                match resolve_bug_column(token) {
                    Some(col) => cols.push(col),
                    None => unknown.push(token),
                }
            }
            if !unknown.is_empty() {
                let _ = writeln!(
                    err,
                    "warning: field(s) not displayable in table output: {}; use --json to see them",
                    unknown.join(", ")
                );
            }
            if cols.is_empty() {
                default_columns()
            } else {
                cols
            }
        }
    };
    if let Some(list) = spec.exclude {
        let excluded: Vec<&'static BugColumn> =
            list.split(',').filter_map(resolve_bug_column).collect();
        columns.retain(|c| !excluded.iter().any(|e| e.header == c.header));
    }
    columns
}

pub fn write_bugs<W: Write + ?Sized, E: Write + ?Sized>(
    bugs: &[Bug],
    spec: ColumnSpec<'_>,
    format: OutputFormat,
    out: &mut W,
    err: &mut E,
) {
    write_formatted(bugs, format, out, |bugs, out| {
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
    });
}

pub fn write_bug_detail<W: Write + ?Sized>(bug: &Bug, format: OutputFormat, out: &mut W) {
    write_formatted(bug, format, out, |bug, out| {
        write_bug_detail_table(bug, out);
    });
}

fn write_bug_detail_table(bug: &Bug, out: &mut (impl Write + ?Sized)) {
    let _ = writeln!(
        out,
        "{} #{}\n{}\n",
        "Bug".bold(),
        bug.id.to_string().bold(),
        bug.summary.bold()
    );
    write_field(out, "Status", &colorize_status(&bug.status));
    write_optional_field(out, "Resolution", bug.resolution.as_deref());
    if let Some(dupe_of) = bug.dupe_of {
        let _ = writeln!(out, "  {:<12}  {dupe_of}", "Duplicate of");
    }
    write_optional_field(out, "Product", bug.product.as_deref());
    write_optional_field(out, "Component", bug.component.as_deref());
    write_optional_field(out, "Assignee", bug.assigned_to.as_deref());
    write_optional_field(out, "Priority", bug.priority.as_deref());
    write_optional_field(out, "Severity", bug.severity.as_deref());
    write_optional_field(out, "Creator", bug.creator.as_deref());
    write_optional_field(out, "Created", bug.creation_time.as_deref());
    write_optional_field(out, "Updated", bug.last_change_time.as_deref());
    write_list_field(out, "Keywords", &bug.keywords);
    write_id_list_field(out, "Blocks", &bug.blocks);
    write_id_list_field(out, "Depends on", &bug.depends_on);
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
/// JSON mode is **not** handled here — the caller emits a
/// `MultiBugViewResult` via `output::write_result`. This function only
/// covers table mode: argument-order detail blocks for `Ok`, visually
/// distinct `UNAVAILABLE` placeholder blocks for `Failed`, with a
/// `─`-divider line between every pair of blocks (no trailing divider).
pub fn write_multi_bug_view<W: Write + ?Sized>(rows: &[MultiBugRow], out: &mut W) {
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            write_divider(out);
        }
        match row {
            MultiBugRow::Ok(bug) => write_bug_detail_table(bug, out),
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
