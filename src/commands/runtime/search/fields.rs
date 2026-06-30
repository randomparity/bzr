use std::collections::HashSet;
use std::io::Write;

use crate::types::bug::{
    apply_exclude, canonical_excludes, default_selected_fields, partition_include, selected_keys,
    BUG_FIELDS,
};
pub(crate) use crate::types::bug::{canonical_field_list, ColumnSpec};

/// Validate that `spec` yields at least one renderable table column. Call
/// ONLY when output is a table, before the network request. Errors (exit 7)
/// when a `--fields` value resolves to zero columns (all tokens unknown), or
/// when `--exclude-fields` removes every column. Partial-unknown (some valid,
/// some not) is allowed and handled as a warning at render time.
pub(crate) fn validate_table_columns(spec: ColumnSpec<'_>) -> crate::error::Result<()> {
    let mut columns = match spec.include {
        None => default_selected_fields(),
        Some(list) => {
            let partition = partition_include(list);
            if partition.ordered.is_empty() {
                if partition.unknown.is_empty() {
                    default_selected_fields()
                } else {
                    return Err(crate::error::BzrError::input(format!(
                        "none of the requested fields are known bug fields: {}",
                        partition.unknown.join(", ")
                    )));
                }
            } else {
                partition.ordered
            }
        }
    };
    apply_exclude(&mut columns, spec.exclude);
    if columns.is_empty() {
        return Err(crate::error::BzrError::input(
            "--exclude-fields removed every table column; nothing left to display".into(),
        ));
    }
    Ok(())
}

/// Validate that `spec` leaves at least one JSON key to emit, measured against
/// the full bug-field universe, not table mode's five-column default. Call ONLY
/// when output is JSON, before the network request.
pub(crate) fn validate_json_field_selection(spec: ColumnSpec<'_>) -> crate::error::Result<()> {
    let mut keys: HashSet<&str> = match spec.include {
        Some(list) => {
            let partition = partition_include(list);
            if partition.ordered.is_empty() && list.split(',').all(|t| t.trim().is_empty()) {
                BUG_FIELDS.iter().map(|field| field.canonical()).collect()
            } else {
                selected_keys(&partition.ordered)
            }
        }
        None => BUG_FIELDS.iter().map(|field| field.canonical()).collect(),
    };
    for canonical in canonical_excludes(spec.exclude) {
        keys.remove(canonical);
    }
    if keys.is_empty() {
        return Err(crate::error::BzrError::input(
            "the field selection leaves no fields to emit; \
         adjust --fields / --exclude-fields"
                .into(),
        ));
    }
    Ok(())
}

/// Warn once on stderr about `--fields` tokens that name no known bug field.
/// Custom `cf_*` fields are dynamic known fields, so only non-custom unknowns
/// reach this warning. Only inspects the include list; unknown
/// `--exclude-fields` tokens are inert and silently ignored.
pub(crate) fn warn_unknown_fields<E: Write + ?Sized>(spec: ColumnSpec<'_>, err: &mut E) {
    let Some(list) = spec.include else {
        return;
    };
    let partition = partition_include(list);
    if !partition.unknown.is_empty() {
        let _ = writeln!(
            err,
            "warning: ignoring unknown field(s): {}",
            partition.unknown.join(", ")
        );
    }
}

#[cfg(test)]
#[path = "fields_tests.rs"]
mod tests;
