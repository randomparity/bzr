/// Overwrite `target` with `value` when an `Option` flag was supplied; an
/// absent flag (`None`) leaves the existing value unchanged. Returns `true` if
/// the flag was supplied (i.e. an assignment happened). Used by the in-place
/// `update` merges for templates and saved queries.
pub(crate) fn merge_set(target: &mut Option<String>, value: Option<&str>) -> bool {
    if let Some(v) = value {
        *target = Some(v.to_owned());
        true
    } else {
        false
    }
}

/// Overwrite `target` with `value` when a repeatable flag was supplied (a
/// non-empty `Vec`); an empty `Vec` (absent flag) leaves it unchanged. Returns
/// `true` if the flag was supplied.
pub(crate) fn merge_vec(target: &mut Vec<String>, value: &[String]) -> bool {
    if value.is_empty() {
        false
    } else {
        *target = value.to_vec();
        true
    }
}
