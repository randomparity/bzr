use crate::cli::SortArgs;
use crate::types::SortDirection;

#[test]
fn explicit_sort_order_is_none_without_sort() {
    let args = SortArgs::default();
    assert_eq!(super::explicit_sort_order(&args), None);
}

#[test]
fn explicit_sort_order_builds_clause_with_tiebreaker() {
    let args = SortArgs {
        sort: Some("priority".into()),
        order: SortDirection::Asc,
    };
    assert_eq!(
        super::explicit_sort_order(&args).as_deref(),
        Some("priority ASC, bug_id")
    );
}

#[test]
fn explicit_sort_order_honors_direction() {
    let args = SortArgs {
        sort: Some("last_change_time".into()),
        order: SortDirection::Desc,
    };
    assert_eq!(
        super::explicit_sort_order(&args).as_deref(),
        Some("last_change_time DESC, bug_id")
    );
}

#[test]
fn explicit_sort_order_omits_redundant_bug_id_tiebreaker() {
    let args = SortArgs {
        sort: Some("bug_id".into()),
        order: SortDirection::Asc,
    };
    assert_eq!(
        super::explicit_sort_order(&args).as_deref(),
        Some("bug_id ASC")
    );
}
