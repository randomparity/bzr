use std::borrow::Cow;

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
