use std::borrow::Cow;

use serde::{Deserialize, Serialize};

/// Known field name aliases mapped to their Bugzilla API internal names.
/// Sorted alphabetically by alias.
///
/// These aliases cannot shadow real Bugzilla field names because Bugzilla
/// requires custom fields to use the `cf_` prefix (e.g. `cf_status`), and
/// the built-in fields have fixed names (e.g. `bug_status`, `priority`).
/// No real field can have a bare name like `status` or `severity`, so eager
/// resolution is always safe.
pub(crate) const FIELD_ALIASES: &[(&str, &str)] = &[
    ("file_loc", "bug_file_loc"),
    ("group", "bug_group"),
    ("id", "bug_id"),
    ("severity", "bug_severity"),
    ("status", "bug_status"),
    ("type", "bug_type"),
];

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FieldValue {
    /// Field value name. Null for the "default/unset" entry in some Bugzilla
    /// field types (e.g. `bug_status` on Bugzilla 5.0 has a null-named entry).
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub sort_key: u64,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub can_change_to: Option<Vec<StatusTransition>>,
}

/// Serde JSON keys of [`FieldValue`], for `--fields` / `--exclude-fields`
/// validation on `field list`.
pub const FIELD_VALUE_FIELDS: &[&str] = &["name", "sort_key", "is_active", "can_change_to"];

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StatusTransition {
    pub name: String,
}

pub(crate) fn resolve_field_alias(name: &str) -> Cow<'_, str> {
    let lower = name.to_ascii_lowercase();
    for &(alias, api_name) in FIELD_ALIASES {
        if lower == alias {
            return Cow::Borrowed(api_name);
        }
    }
    // Unknown fields pass through unchanged; only known aliases are normalized.
    Cow::Borrowed(name)
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
