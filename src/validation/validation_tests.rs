use super::build_order;
use crate::types::SortDirection;

#[test]
fn build_order_defaults_to_bug_id_when_no_sort() {
    assert_eq!(build_order(None, SortDirection::Asc), "bug_id");
    // Direction is ignored without a sort field; the default is always stable.
    assert_eq!(build_order(None, SortDirection::Desc), "bug_id");
}

#[test]
fn build_order_appends_direction_and_tiebreaker() {
    assert_eq!(
        build_order(Some("last_change_time"), SortDirection::Desc),
        "last_change_time DESC, bug_id"
    );
    assert_eq!(
        build_order(Some("priority"), SortDirection::Asc),
        "priority ASC, bug_id"
    );
}

#[test]
fn build_order_omits_redundant_tiebreaker_for_bug_id() {
    assert_eq!(
        build_order(Some("bug_id"), SortDirection::Desc),
        "bug_id DESC"
    );
}

#[test]
fn build_order_resolves_field_aliases() {
    // `severity` is an alias for the API field `bug_severity`.
    assert_eq!(
        build_order(Some("severity"), SortDirection::Asc),
        "bug_severity ASC, bug_id"
    );
    // `id` resolves to `bug_id`, which drops the redundant tiebreaker.
    assert_eq!(build_order(Some("id"), SortDirection::Desc), "bug_id DESC");
}
