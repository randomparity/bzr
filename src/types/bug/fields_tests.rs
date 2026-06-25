use super::*;

#[test]
fn canonical_field_list_translates_aliases() {
    let got = canonical_field_list(Some("assignee,updated,created,reporter,platform"));

    assert_eq!(
        got.as_deref(),
        Some("assigned_to,last_change_time,creation_time,creator,rep_platform")
    );
}

#[test]
fn partition_include_dedupes_aliases_and_preserves_custom_fields() {
    let partition = partition_include("assignee,assigned_to,cf_release,not_a_field");

    assert_eq!(
        partition.ordered,
        vec![
            SelectedBugField::BuiltIn(BugField::AssignedTo),
            SelectedBugField::Custom("cf_release")
        ]
    );
    assert_eq!(partition.custom, vec!["cf_release"]);
    assert_eq!(partition.unknown, vec!["not_a_field"]);
}

#[test]
fn field_selected_treats_aliases_as_same_field() {
    let spec = ColumnSpec::new(Some("assignee"), Some("assigned_to"));

    assert!(!field_selected(spec, "assigned_to"));
}
