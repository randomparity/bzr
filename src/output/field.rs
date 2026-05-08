use std::io::Write;

use serde::Serialize;
use tabled::{Table, Tabled};

use super::formatting::{write_formatted, yes_no};
use crate::types::{FieldValue, OutputFormat};

#[derive(Tabled)]
struct FieldValueRow {
    #[tabled(rename = "NAME")]
    name: String,
    #[tabled(rename = "ACTIVE")]
    active: String,
    #[tabled(rename = "CAN CHANGE TO")]
    can_change_to: String,
}

pub fn write_field_values<W: Write + ?Sized>(
    values: &[FieldValue],
    format: OutputFormat,
    out: &mut W,
) {
    write_formatted(values, format, out, |values, out| {
        let rows: Vec<FieldValueRow> = values
            .iter()
            .map(|v| {
                let transitions = v
                    .can_change_to
                    .as_ref()
                    .map(|t| {
                        t.iter()
                            .map(|s| s.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                FieldValueRow {
                    name: v.name.clone(),
                    active: yes_no(v.is_active).into(),
                    can_change_to: transitions,
                }
            })
            .collect();
        let _ = writeln!(out, "{}", Table::new(rows));
    });
}

#[derive(Serialize, Tabled)]
struct FieldAliasRow {
    #[tabled(rename = "ALIAS")]
    alias: &'static str,
    #[tabled(rename = "API FIELD NAME")]
    api_name: &'static str,
}

pub fn write_field_aliases<W: Write + ?Sized>(
    aliases: &[(&'static str, &'static str)],
    format: OutputFormat,
    out: &mut W,
) {
    let rows: Vec<FieldAliasRow> = aliases
        .iter()
        .map(|&(alias, api_name)| FieldAliasRow { alias, api_name })
        .collect();
    write_formatted(&rows, format, out, |rows, out| {
        let _ = writeln!(out, "{}", Table::new(rows));
    });
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
