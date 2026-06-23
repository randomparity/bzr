/// Apply CLI-over-stored precedence for optional scalar fields.
///
/// A supplied CLI flag wins; an absent CLI flag leaves the existing JSON,
/// template, or saved-query value unchanged.
pub(crate) fn merge_set(target: &mut Option<String>, value: Option<&str>) -> bool {
    if let Some(v) = value {
        *target = Some(v.to_owned());
        true
    } else {
        false
    }
}

/// Apply CLI-over-stored precedence for repeatable string fields.
///
/// A non-empty CLI vector wins; an empty vector means the flag was absent.
pub(crate) fn merge_vec(target: &mut Vec<String>, value: &[String]) -> bool {
    if value.is_empty() {
        false
    } else {
        *target = value.to_vec();
        true
    }
}
