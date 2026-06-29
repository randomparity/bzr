use crate::error::{BzrError, Result};
use crate::types::bug::SearchParams;

/// Rewrite search params for `--count`: fetch only IDs and lift row limits so
/// the count reflects the full match set rather than one page.
pub(crate) fn count_search_params(mut params: SearchParams) -> SearchParams {
    params.include_fields = Some("id".to_string());
    params.exclude_fields = None;
    params.limit = Some(0);
    params.offset = None;
    params.raw_params.retain(|(key, _)| key != "offset");
    params
}

/// Reject page-windowing flags with `--count`, where a full match total is the
/// only coherent result.
pub(crate) fn ensure_no_paging_with_count(
    count: bool,
    offset: Option<u32>,
    paginate: bool,
) -> Result<()> {
    if count && (offset.is_some() || paginate) {
        return Err(BzrError::input(
            "--count cannot be combined with --offset or --paginate".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
