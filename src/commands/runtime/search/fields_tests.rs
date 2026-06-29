use super::*;

#[test]
fn validate_table_columns_errors_for_all_unknown_include() {
    let spec = ColumnSpec::new(Some("not_a_field,also_not_a_field"), None);

    assert!(
        matches!(
            validate_table_columns(spec),
            Err(crate::error::BzrError::InputValidation { message: ref msg, .. })
                if msg.contains("none of the requested fields are known bug fields")
        ),
        "all-unknown table field selection should be rejected"
    );
}

#[test]
fn validate_json_field_selection_allows_non_default_builtin_fields() {
    let spec = ColumnSpec::new(
        Some("severity"),
        Some("id,status,priority,assigned_to,summary"),
    );

    assert!(validate_json_field_selection(spec).is_ok());
}

#[test]
fn warn_unknown_fields_reports_only_unknown_include_tokens() {
    let mut err = Vec::new();
    let spec = ColumnSpec::new(Some("summary,cf_release,not_a_field"), Some("also_ignored"));

    warn_unknown_fields(spec, &mut err);

    let warning = String::from_utf8_lossy(&err);
    assert!(
        warning.contains("ignoring unknown field(s): not_a_field"),
        "warning text: {warning}"
    );
    assert!(
        !warning.contains("also_ignored"),
        "exclude-only unknowns should be inert: {warning}"
    );
}
