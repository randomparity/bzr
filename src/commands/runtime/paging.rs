//! Shared result paging for the search-backed bug commands (`list`, `search`,
//! `my`) and `query run`.
//!
//! Bugzilla's `Bug.search` returns no total-match count, so truncation cannot
//! be read from the response. Two mechanisms cover the gap:
//!
//! - **`--offset <N>`** skips leading matches, so a window past the first
//!   `--limit` is retrievable.
//! - **`--paginate`** loops `offset += limit` until a short page and returns
//!   every match — the full-retrieval path for "process all matching bugs".
//!
//! Without `--paginate`, [`fetch_page`] over-fetches one extra row (`limit+1`):
//! if the server returns more than `limit`, at least one more match exists, so
//! `truncated` is set deterministically and the surplus row trimmed away.

use crate::client::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::output::writers::Writers;
use crate::types::bug::{Bug, SearchParams};
use crate::types::output::OutputFormat;

/// Backstop on the `--paginate` loop: a server that ignored `offset` would
/// otherwise return a full page forever. Far above any real result set.
const MAX_PAGES: u32 = 10_000;

/// Resolve the effective `offset` for a search. A `--from-url` (or a saved
/// query imported from one) may carry its own `offset=` as an unrecognized
/// param living in `raw_params`; fold it into the struct field — the single
/// source of truth the client appends and the `--paginate` loop steps — strip
/// the raw copy so a request never sends two conflicting `offset` params, then
/// let an explicit CLI `--offset` override. Mirrors how a URL's `limit` is
/// folded into the structured field.
pub(crate) fn resolve_offset(params: &mut SearchParams, cli_offset: Option<u32>) {
    let url_offset = params
        .raw_params
        .iter()
        .find(|(k, _)| k == "offset")
        .and_then(|(_, v)| v.parse::<u32>().ok());
    params.raw_params.retain(|(k, _)| k != "offset");
    params.offset = cli_offset.or(url_offset);
}

/// A page of results plus whether more matches exist beyond it.
pub(crate) struct Page {
    pub bugs: Vec<Bug>,
    pub truncated: bool,
}

fn overfetch_limit(limit: u32) -> Result<u32> {
    limit.checked_add(1).ok_or_else(|| {
        BzrError::InputValidation("--limit is too large to detect additional search results".into())
    })
}

fn checked_next_offset(offset: u32, limit: u32) -> Result<u32> {
    offset.checked_add(limit).ok_or_else(|| {
        BzrError::InputValidation(format!(
            "--offset {offset} plus --limit {limit} is too large to calculate the next page"
        ))
    })
}

/// Fetch results honoring `--offset`/`--paginate`. With `paginate`, returns
/// every match or errors if the safety cap is reached before a short page.
/// Otherwise returns one window, with `truncated` set when the server had more
/// rows than `limit`.
pub(crate) async fn fetch_page(
    client: &BugzillaClient,
    params: &SearchParams,
    paginate: bool,
) -> Result<Page> {
    if paginate {
        let bugs = fetch_all_pages(client, params).await?;
        return Ok(Page {
            bugs,
            truncated: false,
        });
    }
    // Over-fetch one row past the limit to detect "more available" without a
    // server total. A zero/absent limit means "no window", so nothing to flag.
    let Some(limit) = params.limit.filter(|l| *l > 0) else {
        let bugs = client.search_bugs(params).await?;
        return Ok(Page {
            bugs,
            truncated: false,
        });
    };
    let mut probe = params.clone();
    probe.limit = Some(overfetch_limit(limit)?);
    let _ = checked_next_offset(params.offset.unwrap_or(0), limit)?;
    let mut bugs = client.search_bugs(&probe).await?;
    let truncated = bugs.len() as u64 > u64::from(limit);
    bugs.truncate(limit as usize);
    Ok(Page { bugs, truncated })
}

/// Loop `offset += limit` until a page shorter than `limit` (the last page) and
/// return the accumulated matches. A zero/absent limit means the single
/// unbounded request already returns everything (bounded by the server max).
async fn fetch_all_pages(client: &BugzillaClient, params: &SearchParams) -> Result<Vec<Bug>> {
    fetch_all_pages_with_cap(client, params, MAX_PAGES).await
}

async fn fetch_all_pages_with_cap(
    client: &BugzillaClient,
    params: &SearchParams,
    max_pages: u32,
) -> Result<Vec<Bug>> {
    let Some(page_size) = params.limit.filter(|l| *l > 0) else {
        return client.search_bugs(params).await;
    };
    let mut offset = params.offset.unwrap_or(0);
    let mut all = Vec::new();
    let mut reached_last_page = false;
    for _ in 0..max_pages {
        let next_offset = checked_next_offset(offset, page_size)?;
        let mut p = params.clone();
        p.offset = Some(offset);
        let batch = client.search_bugs(&p).await?;
        let received = batch.len();
        all.extend(batch);
        if received < page_size as usize {
            reached_last_page = true;
            break;
        }
        offset = next_offset;
    }
    if !reached_last_page {
        return Err(BzrError::InputValidation(format!(
            "--paginate stopped at the {max_pages}-page safety cap before reaching a short page; \
             the server may be ignoring offset, so results would be incomplete"
        )));
    }
    Ok(all)
}

/// Emit a truncation footer when a window was truncated: a visible footer line
/// on stdout for table output, or a note on stderr for JSON (so stdout stays a
/// clean, parseable document). No-op when nothing was truncated.
pub(crate) fn write_truncation_note(
    page: &Page,
    limit: Option<u32>,
    offset: Option<u32>,
    format: OutputFormat,
    w: &mut Writers<'_>,
) {
    if !page.truncated {
        return;
    }
    let Some(limit) = limit else { return };
    let Some(next) = offset.unwrap_or(0).checked_add(limit) else {
        return;
    };
    let msg = format!(
        "Showing first {} result(s); more available — use --paginate for all, \
         or --offset {next} for the next page.",
        page.bugs.len()
    );
    match format {
        OutputFormat::Table => {
            let _ = writeln!(w.out, "{msg}");
        }
        // For machine output the note goes to stderr so stdout stays a clean,
        // parseable document (pretty JSON or one NDJSON record per line).
        OutputFormat::Json | OutputFormat::Ndjson => {
            let _ = writeln!(w.err, "{msg}");
        }
    }
}

#[cfg(test)]
#[path = "paging_tests.rs"]
mod tests;
