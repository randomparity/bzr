use std::io::Write;

use serde::Serialize;

use crate::output::formatting::{
    opt_yes_no, write_formatted, write_formatted_projected, write_table_records,
};
use crate::types::{FieldName, FieldValue, OutputFormat};
use crate::validation::fields::FieldProjection;

const FIELD_VALUE_HEADERS: &[&str] = &["NAME", "ACTIVE", "CAN CHANGE TO"];
const FIELD_ALIAS_HEADERS: &[&str] = &["ALIAS", "API FIELD NAME"];
const FIELD_NAME_HEADERS: &[&str] = &["NAME", "SOURCE"];

fn field_value_record(value: &FieldValue) -> Vec<String> {
    let transitions = value
        .can_change_to
        .as_ref()
        .map(|transitions| {
            transitions
                .iter()
                .map(|transition| transition.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    vec![
        value.name.clone().unwrap_or_default(),
        opt_yes_no(value.is_active).into(),
        transitions,
    ]
}

pub fn write_field_values<W: Write + ?Sized>(
    values: &[FieldValue],
    format: OutputFormat,
    projection: &FieldProjection,
    table_width: Option<usize>,
    out: &mut W,
) {
    write_formatted_projected(values, format, projection, out, |values, out| {
        write_table_records(
            FIELD_VALUE_HEADERS,
            values.iter().map(field_value_record),
            table_width,
            out,
        );
    });
}

/// Render the bug field names a `--field` write accepts. `source` says why each
/// one is accepted; see ADR 0062.
///
/// The table cell comes from `FieldNameSource::as_str`, the same definition
/// serde serializes through, so the table and JSON spellings cannot diverge.
pub fn write_field_names<W: Write + ?Sized>(
    names: &[FieldName],
    format: OutputFormat,
    projection: &FieldProjection,
    table_width: Option<usize>,
    out: &mut W,
) {
    write_formatted_projected(names, format, projection, out, |names, out| {
        write_table_records(
            FIELD_NAME_HEADERS,
            names
                .iter()
                .map(|row| vec![row.name.clone(), row.source.as_str().to_string()]),
            table_width,
            out,
        );
    });
}

#[derive(Serialize)]
struct FieldAliasRow {
    alias: &'static str,
    api_name: &'static str,
}

pub fn write_field_aliases<W: Write + ?Sized>(
    aliases: &[(&'static str, &'static str)],
    format: OutputFormat,
    table_width: Option<usize>,
    out: &mut W,
) {
    let rows: Vec<FieldAliasRow> = aliases
        .iter()
        .map(|&(alias, api_name)| FieldAliasRow { alias, api_name })
        .collect();
    write_formatted(&rows, format, out, |rows, out| {
        write_table_records(
            FIELD_ALIAS_HEADERS,
            rows.iter()
                .map(|row| vec![row.alias.to_string(), row.api_name.to_string()]),
            table_width,
            out,
        );
    });
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
