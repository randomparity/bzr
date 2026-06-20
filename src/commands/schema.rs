//! `bzr schema` — publish JSON Schemas for the tool's JSON output shapes.
//!
//! Purely local: no config, network, or auth. Each schema describes the
//! `--format json` body of a command family so agents can validate output
//! against a contract instead of branching per command. The schema files are
//! checked into `schemas/` at the repo root and embedded here at build time;
//! a drift test (see `schema_tests.rs`) validates representative serialized
//! values against them, so a struct change that breaks a schema fails CI.

use crate::error::{BzrError, Result};
use crate::output::result_types::write_result;
use crate::output::writers::Writers;
use crate::types::OutputFormat;

/// Every published schema as `(name, schema-json)`, embedded from `schemas/`.
/// Names are agent-facing and stable; keep this list sorted for the `schema`
/// listing and the not-found hint.
pub(crate) const SCHEMAS: &[(&str, &str)] = &[
    (
        "action-result",
        include_str!("../../schemas/action-result.json"),
    ),
    ("attachment", include_str!("../../schemas/attachment.json")),
    (
        "batch-create-result",
        include_str!("../../schemas/batch-create-result.json"),
    ),
    (
        "batch-result",
        include_str!("../../schemas/batch-result.json"),
    ),
    ("bug", include_str!("../../schemas/bug.json")),
    (
        "classification",
        include_str!("../../schemas/classification.json"),
    ),
    ("comment", include_str!("../../schemas/comment.json")),
    ("component", include_str!("../../schemas/component.json")),
    (
        "config-result",
        include_str!("../../schemas/config-result.json"),
    ),
    (
        "count-result",
        include_str!("../../schemas/count-result.json"),
    ),
    (
        "download-result",
        include_str!("../../schemas/download-result.json"),
    ),
    (
        "dry-run-result",
        include_str!("../../schemas/dry-run-result.json"),
    ),
    (
        "field-value",
        include_str!("../../schemas/field-value.json"),
    ),
    ("group", include_str!("../../schemas/group.json")),
    (
        "membership-result",
        include_str!("../../schemas/membership-result.json"),
    ),
    (
        "multi-bug-view",
        include_str!("../../schemas/multi-bug-view.json"),
    ),
    ("product", include_str!("../../schemas/product.json")),
    (
        "search-result",
        include_str!("../../schemas/search-result.json"),
    ),
    ("tag-result", include_str!("../../schemas/tag-result.json")),
    (
        "upload-result",
        include_str!("../../schemas/upload-result.json"),
    ),
    ("user", include_str!("../../schemas/user.json")),
];

/// Look up a schema by name.
fn find(name: &str) -> Option<&'static str> {
    SCHEMAS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, body)| *body)
}

/// Print a published JSON Schema, or the list of available schema names when
/// `name` is `None`.
pub fn execute(name: Option<&str>, format: OutputFormat, w: &mut Writers<'_>) -> Result<()> {
    let Some(name) = name else {
        write_list(format, w);
        return Ok(());
    };
    write_one(name, w)
}

/// Write a single named schema verbatim. A schema is itself a JSON document, so
/// it is emitted as-is regardless of `--format` (no table/NDJSON projection).
fn write_one(name: &str, w: &mut Writers<'_>) -> Result<()> {
    let Some(body) = find(name) else {
        return Err(BzrError::InputValidation(format!(
            "unknown schema '{name}'; available: {}",
            available_names().join(", ")
        )));
    };
    // Embedded files already end with a newline; write verbatim.
    let _ = write!(w.out, "{body}");
    Ok(())
}

/// List available schema names: one per line for table, a JSON array for
/// `--json`, one name per line for `--ndjson`.
fn write_list(format: OutputFormat, w: &mut Writers<'_>) {
    let names = available_names();
    let table = names.join("\n");
    write_result(&names, &table, format, w.out);
}

/// The published schema names, in registry order.
fn available_names() -> Vec<&'static str> {
    SCHEMAS.iter().map(|(name, _)| *name).collect()
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
