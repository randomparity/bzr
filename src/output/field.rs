use std::io::{self, Write as _};

use serde::Serialize;
use tabled::{Table, Tabled};

use super::formatting::{print_formatted, yes_no};
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

pub fn print_field_values(values: &[FieldValue], format: OutputFormat) {
    print_formatted(values, format, |values| {
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
        let _ = writeln!(io::stdout(), "{}", Table::new(rows));
    });
}

#[derive(Serialize, Tabled)]
struct FieldAliasRow {
    #[tabled(rename = "ALIAS")]
    alias: &'static str,
    #[tabled(rename = "API FIELD NAME")]
    api_name: &'static str,
}

pub fn print_field_aliases(aliases: &[(&'static str, &'static str)], format: OutputFormat) {
    let rows: Vec<FieldAliasRow> = aliases
        .iter()
        .map(|&(alias, api_name)| FieldAliasRow { alias, api_name })
        .collect();
    print_formatted(&rows, format, |rows| {
        let _ = writeln!(io::stdout(), "{}", Table::new(rows));
    });
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
