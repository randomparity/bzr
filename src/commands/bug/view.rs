use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::output::{
    self, write_multi_bug_view, BugViewFailure, MultiBugRow, MultiBugViewResult, Writers,
};
use crate::types::{Bug, OutputFormat};

#[derive(Clone, Copy)]
enum BugViewMode {
    Strict,
    Permissive,
}

struct BugViewBatch {
    rows: Vec<BugViewRow>,
}

enum BugViewRow {
    Ok(Box<Bug>),
    Failed(BugViewFailure),
}

impl BugViewBatch {
    fn into_json_result(self) -> MultiBugViewResult {
        let mut bugs = Vec::with_capacity(self.rows.len());
        let mut failed = Vec::new();
        for row in self.rows {
            match row {
                BugViewRow::Ok(bug) => bugs.push(*bug),
                BugViewRow::Failed(failure) => failed.push(failure),
            }
        }
        MultiBugViewResult { bugs, failed }
    }

    fn into_table_rows(self) -> Vec<MultiBugRow> {
        self.rows
            .into_iter()
            .map(|row| match row {
                BugViewRow::Ok(bug) => MultiBugRow::Ok(bug),
                BugViewRow::Failed(failure) => MultiBugRow::Failed {
                    id: failure.id,
                    error: failure.error,
                },
            })
            .collect()
    }
}

pub(super) async fn handle(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let BugAction::View {
        ids,
        permissive,
        fields,
        exclude_fields,
    } = action
    else {
        unreachable!()
    };

    if *permissive && ids.len() == 1 {
        return Err(BzrError::InputValidation(
            "--permissive only meaningful with multiple IDs".into(),
        ));
    }

    let inc = fields.as_deref();
    let exc = exclude_fields.as_deref();

    if ids.len() == 1 {
        return view_single(client, &ids[0], inc, exc, format, w).await;
    }

    let mode = if *permissive {
        BugViewMode::Permissive
    } else {
        BugViewMode::Strict
    };
    let batch = fetch_batch(client, ids, inc, exc, mode).await?;
    write_batch(batch, format, w);
    Ok(())
}

async fn view_single(
    client: &BugzillaClient,
    id: &str,
    include_fields: Option<&str>,
    exclude_fields: Option<&str>,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let bug = client.get_bug(id, include_fields, exclude_fields).await?;
    output::write_bug_detail(&bug, format, w.out);
    Ok(())
}

async fn fetch_batch(
    client: &BugzillaClient,
    ids: &[String],
    include_fields: Option<&str>,
    exclude_fields: Option<&str>,
    mode: BugViewMode,
) -> Result<BugViewBatch> {
    let mut rows = Vec::with_capacity(ids.len());
    for id in ids {
        match client.get_bug(id, include_fields, exclude_fields).await {
            Ok(bug) => rows.push(BugViewRow::Ok(Box::new(bug))),
            Err(e) if matches!(mode, BugViewMode::Permissive) && e.is_bug_get_per_resource() => {
                rows.push(BugViewRow::Failed(BugViewFailure {
                    id: id.clone(),
                    error: e.to_string(),
                }));
            }
            Err(e) => return Err(e),
        }
    }
    Ok(BugViewBatch { rows })
}

fn write_batch(batch: BugViewBatch, format: OutputFormat, w: &mut Writers<'_>) {
    match format {
        OutputFormat::Table => {
            let rows = batch.into_table_rows();
            write_multi_bug_view(&rows, w.out);
        }
        OutputFormat::Json => {
            let result = batch.into_json_result();
            output::write_result(&result, "", format, w.out);
        }
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
