use std::collections::HashMap;

use crate::cli::HistoryArgs;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::resources::bug::{write_history_json, write_history_table};
use crate::output::writers::Writers;
use crate::types::output::OutputFormat;
use crate::types::{Comment, HistoryEntry, HistoryRecord};
use crate::validation::{parse_optional_date, timestamp_compare_key};

pub(super) async fn handle(
    client: &BugzillaClient,
    args: &HistoryArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let HistoryArgs { id, since } = args;

    let canonical_since = parse_optional_date(since.as_deref(), "--since")?;

    let history = client
        .get_bug_history_since(*id, canonical_since.as_deref())
        .await?;

    if format.is_json_family() {
        // Correlate `comment_id` from the full comment set (unfiltered by
        // `--since`, which scopes only the history records). Best-effort: a
        // comment-fetch failure degrades to `comment_id: null` with a warning
        // rather than failing the command — see ADR 0008. Skip the fetch
        // entirely when there is no history to correlate against.
        let records = if history.is_empty() {
            Vec::new()
        } else {
            let comments = fetch_comments_for_correlation(client, *id, w).await;
            flatten_history(&history, &comments)
        };
        write_history_json(&records, format, w.out);
    } else if history.is_empty() {
        let _ = writeln!(w.out, "No history for bug #{id}.");
    } else {
        write_history_table(&history, w.out);
    }
    Ok(())
}

/// Fetch the bug's comments for `comment_id` correlation, degrading to an empty
/// set (with a stderr warning) on failure. The change delta is the contract;
/// comment correlation is best-effort enrichment.
async fn fetch_comments_for_correlation(
    client: &BugzillaClient,
    id: u64,
    w: &mut Writers<'_>,
) -> Vec<Comment> {
    match client.get_comments_since(id, None).await {
        Ok(comments) => comments,
        Err(e) => {
            let _ = writeln!(
                w.err,
                "warning: could not fetch comments for comment_id correlation \
                 (comment_id will be null): {e}"
            );
            Vec::new()
        }
    }
}

/// Flatten grouped history entries into one record per changed field, sharing
/// each entry's `when`/`who`/`comment_id`. `comment_id` is correlated by exact
/// `who == creator` plus a canonical timestamp-key match; it can miss (→ null)
/// but never produces a wrong id. On a duplicate `(who, when-key)` the smallest
/// comment id wins.
fn flatten_history(entries: &[HistoryEntry], comments: &[Comment]) -> Vec<HistoryRecord> {
    let mut index: HashMap<(String, String), u64> = HashMap::new();
    for comment in comments {
        let (Some(creator), Some(when)) =
            (comment.creator.as_deref(), comment.creation_time.as_deref())
        else {
            continue;
        };
        let Some(key) = timestamp_compare_key(when) else {
            continue;
        };
        index
            .entry((creator.to_string(), key))
            .and_modify(|id| *id = (*id).min(comment.id))
            .or_insert(comment.id);
    }

    let mut records = Vec::new();
    for entry in entries {
        let comment_id = timestamp_compare_key(&entry.when)
            .and_then(|key| index.get(&(entry.who.clone(), key)).copied());
        for change in &entry.changes {
            records.push(HistoryRecord {
                when: entry.when.clone(),
                who: entry.who.clone(),
                field: change.field_name.clone(),
                old_value: change.removed.clone(),
                new_value: change.added.clone(),
                comment_id,
            });
        }
    }
    records
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
