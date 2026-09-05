//! Shared base setup for the bug listing commands (`bug list`, `bug my`).
//!
//! Both commands build the same `SearchParams` skeleton (limit/offset window,
//! canonical include/exclude field lists, parsed date filters, sort order) and
//! render results through the same output seam (rows plus the truncation
//! footer). Only the match criteria differ — `bug list` adds id/alias/summary
//! and actor filters, `bug my` runs the assigned/created/cc categories and
//! dedups. This module owns the shared seams; the command-specific query logic
//! stays in each command.

use crate::commands::runtime::search::fields::{canonical_field_list, ColumnSpec};
use crate::commands::runtime::search::paging::{write_truncation_note, Page};
use crate::error::Result;
use crate::output::resources::bug::write_bugs;
use crate::output::writers::Writers;
use crate::types::bug::SearchParams;
use crate::types::output::OutputFormat;
use crate::types::SortDirection;
use crate::validation::{build_order, parse_optional_date};

/// The field/date/sort/page inputs shared by `bug list` and `bug my` when
/// building the base `SearchParams`. Command-specific match criteria
/// (quicksearch ids/alias/summary, actor filters, per-category
/// assignee/creator/cc) are applied by the caller after construction.
#[derive(Clone, Copy)]
pub(crate) struct BaseSearchInputs<'a> {
    pub limit: u32,
    pub offset: Option<u32>,
    pub fields: Option<&'a str>,
    pub exclude_fields: Option<&'a str>,
    pub created_since: Option<&'a str>,
    pub changed_since: Option<&'a str>,
    pub sort: Option<&'a str>,
    pub order: SortDirection,
}

/// Build the `SearchParams` common to the bug listing commands: the limit/offset
/// window, canonical include/exclude field lists, parsed `--created-since` /
/// `--changed-since` filters, and the sort order. All command-specific match
/// criteria are left at their `Default` for the caller to fill in.
///
/// # Errors
///
/// Returns [`crate::error::BzrError::InputValidation`] when `created_since` or
/// `changed_since` is not a recognized date, naming the offending flag.
pub(crate) fn build_base_search_params(inputs: BaseSearchInputs<'_>) -> Result<SearchParams> {
    let creation_time = parse_optional_date(inputs.created_since, "--created-since")?;
    let last_change_time = parse_optional_date(inputs.changed_since, "--changed-since")?;

    Ok(SearchParams {
        limit: Some(inputs.limit),
        offset: inputs.offset,
        include_fields: canonical_field_list(inputs.fields),
        exclude_fields: canonical_field_list(inputs.exclude_fields),
        creation_time,
        last_change_time,
        order: Some(build_order(inputs.sort, inputs.order)),
        ..Default::default()
    })
}

/// The `--limit` / `--offset` window used to phrase the truncation footer.
#[derive(Clone, Copy)]
pub(crate) struct PageWindow {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Write the bug rows followed by the shared truncation footer. The single
/// output seam for `bug list` and `bug my`, so both render results and the
/// "more available" note identically.
pub(crate) fn write_bug_page(
    page: &Page,
    spec: ColumnSpec<'_>,
    window: PageWindow,
    format: OutputFormat,
    w: &mut Writers<'_>,
) {
    write_bugs(&page.bugs, spec, format, w.table_width(), w.out, w.err);
    write_truncation_note(page, window.limit, window.offset, format, w);
}

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
