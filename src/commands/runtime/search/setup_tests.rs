#![expect(clippy::unwrap_used)]

use crate::commands::runtime::search::setup::{build_base_search_params, BaseSearchInputs};
use crate::types::SortDirection;

fn inputs() -> BaseSearchInputs<'static> {
    BaseSearchInputs {
        limit: 50,
        offset: None,
        fields: None,
        exclude_fields: None,
        created_since: None,
        changed_since: None,
        sort: None,
        order: SortDirection::Asc,
    }
}

#[test]
fn sets_limit_and_passes_offset_through() {
    let params = build_base_search_params(BaseSearchInputs {
        offset: Some(25),
        ..inputs()
    })
    .unwrap();

    assert_eq!(params.limit, Some(50));
    assert_eq!(params.offset, Some(25));
}

#[test]
fn canonicalizes_field_aliases() {
    let params = build_base_search_params(BaseSearchInputs {
        fields: Some("assignee,updated"),
        exclude_fields: Some("reporter"),
        ..inputs()
    })
    .unwrap();

    assert_eq!(
        params.include_fields.as_deref(),
        Some("assigned_to,last_change_time")
    );
    assert_eq!(params.exclude_fields.as_deref(), Some("creator"));
}

#[test]
fn default_sort_is_stable_bug_id() {
    let params = build_base_search_params(inputs()).unwrap();
    assert_eq!(params.order.as_deref(), Some("bug_id"));
}

#[test]
fn sort_field_appends_direction_and_tiebreaker() {
    let params = build_base_search_params(BaseSearchInputs {
        sort: Some("priority"),
        order: SortDirection::Desc,
        ..inputs()
    })
    .unwrap();

    assert_eq!(params.order.as_deref(), Some("priority DESC, bug_id"));
}

#[test]
fn parses_date_filters_into_params() {
    let params = build_base_search_params(BaseSearchInputs {
        created_since: Some("2025-01-01"),
        changed_since: Some("2025-06-01"),
        ..inputs()
    })
    .unwrap();

    assert!(params.creation_time.is_some());
    assert!(params.last_change_time.is_some());
}

#[test]
fn invalid_created_since_errors_naming_the_flag() {
    let err = build_base_search_params(BaseSearchInputs {
        created_since: Some("not-a-date"),
        ..inputs()
    })
    .unwrap_err();

    assert!(err.to_string().contains("--created-since"), "got: {err}");
}

#[test]
fn leaves_command_specific_criteria_at_default() {
    let params = build_base_search_params(inputs()).unwrap();

    assert!(params.id.is_empty());
    assert!(params.alias.is_none());
    assert!(params.summary.is_none());
    assert!(params.assigned_to.is_empty());
    assert!(params.creator.is_empty());
    assert!(params.cc.is_none());
    assert!(params.quicksearch.is_none());
}
