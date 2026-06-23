use super::*;

#[test]
fn count_search_params_fetches_all_ids_without_saved_offsets() {
    let params = SearchParams {
        include_fields: Some("summary,status".to_string()),
        exclude_fields: Some("priority".to_string()),
        limit: Some(25),
        offset: Some(50),
        raw_params: vec![
            ("offset".to_string(), "50".to_string()),
            ("bug_status".to_string(), "NEW".to_string()),
        ],
        ..Default::default()
    };

    let params = count_search_params(params);

    assert_eq!(params.include_fields.as_deref(), Some("id"));
    assert_eq!(params.exclude_fields, None);
    assert_eq!(params.limit, Some(0));
    assert_eq!(params.offset, None);
    assert_eq!(
        params.raw_params,
        vec![("bug_status".to_string(), "NEW".to_string())]
    );
}

#[test]
fn count_search_rejects_offset_or_pagination() {
    assert!(ensure_no_paging_with_count(true, Some(5), false).is_err());
    assert!(ensure_no_paging_with_count(true, None, true).is_err());
    assert!(ensure_no_paging_with_count(true, None, false).is_ok());
    assert!(ensure_no_paging_with_count(false, Some(5), true).is_ok());
}
