//! `bzr schema` — publish JSON Schemas for the tool's JSON contracts.
//!
//! Purely local: no config, network, or auth. Each schema describes either a
//! `--json` output body or a `--from-json` input payload so agents can
//! validate against a contract instead of branching per command. The schema
//! files are checked into `schemas/` at the repo root and embedded here at build
//! time; drift tests validate representative serialized values and structured
//! input parsers against them, so a contract change fails CI until the schema is
//! updated.

use crate::commands::runtime::context::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::result_types::write_result;
use crate::output::writers::Writers;
use crate::types::output::OutputFormat;

/// Build the `(name, embedded-json)` registry from a bare list of schema names,
/// deriving each `schemas/<name>.json` path so a name is written exactly once.
macro_rules! schema_registry {
    ($($name:literal),* $(,)?) => {
        &[ $( ($name, include_str!(concat!("../../schemas/", $name, ".json"))) ),* ]
    };
}

/// Every published schema as `(name, schema-json)`, embedded from `schemas/`.
/// Names are agent-facing and stable; keep this list sorted for the `schema`
/// listing and the not-found hint.
pub(crate) const SCHEMAS: &[(&str, &str)] = schema_registry![
    "action-result",
    "attachment",
    "batch-create-result",
    "batch-result",
    "bug",
    "bug-create-input",
    "bug-update-input",
    "classification",
    "comment",
    "component",
    "component-create-input",
    "component-update-input",
    "config-result",
    "count-result",
    "download-result",
    "dry-run-result",
    "error",
    "field-value",
    "group",
    "group-create-input",
    "group-update-input",
    "membership-result",
    "multi-bug-view",
    "product",
    "product-create-input",
    "product-update-input",
    "search-result",
    "server-capabilities",
    "tag-result",
    "upload-result",
    "user",
    "user-create-input",
    "user-update-input",
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
#[expect(
    clippy::unused_async,
    reason = "command handlers share the async dispatch signature"
)]
pub async fn execute(name: Option<&str>, ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let format = ctx.format();
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
/// `--json`, one JSON string per line for `--output ndjson`.
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
