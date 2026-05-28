use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::output::resources::bug::{
    bug_to_json, canonical_field_list, write_bug_detail, write_multi_bug_view, ColumnSpec,
    MultiBugRow,
};
use crate::output::result_types::{write_result, BugViewFailure, MultiBugViewResult};
use crate::output::writers::Writers;
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

    // Raw values drive column / detail-row selection (aliases resolve fine);
    // canonical values go to the server so aliased fields are populated.
    let inc = fields.as_deref();
    let exc = exclude_fields.as_deref();
    let inc_canonical = canonical_field_list(inc);
    let exc_canonical = canonical_field_list(exc);

    if ids.len() == 1 {
        let spec = ColumnSpec {
            include: inc_canonical.as_deref(),
            exclude: exc_canonical.as_deref(),
        };
        return view_single(client, &ids[0], spec, format, w).await;
    }

    let mode = if *permissive {
        BugViewMode::Permissive
    } else {
        BugViewMode::Strict
    };
    let batch = fetch_batch(
        client,
        ids,
        inc_canonical.as_deref(),
        exc_canonical.as_deref(),
        mode,
    )
    .await?;
    let spec = ColumnSpec {
        include: inc,
        exclude: exc,
    };
    write_batch(batch, spec, format, w);
    Ok(())
}

async fn view_single(
    client: &BugzillaClient,
    id: &str,
    spec: ColumnSpec<'_>,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let bug = client.get_bug(id, spec.include, spec.exclude).await?;
    write_bug_detail(&bug, spec, format, w.out);
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

fn write_batch(
    batch: BugViewBatch,
    spec: ColumnSpec<'_>,
    format: OutputFormat,
    w: &mut Writers<'_>,
) {
    match format {
        OutputFormat::Table => {
            let rows = batch.into_table_rows();
            write_multi_bug_view(&rows, spec, w.out);
        }
        OutputFormat::Json => {
            // Project each bug to the selected fields; the wrapper keys and the
            // per-failure metadata (`id`, `error`) stay untrimmed so `jq`
            // consumers can always rely on `.bugs[]` / `.failed[]`.
            let result = batch.into_json_result();
            let bugs: Vec<serde_json::Value> =
                result.bugs.iter().map(|b| bug_to_json(b, spec)).collect();
            let value = serde_json::json!({ "bugs": bugs, "failed": result.failed });
            write_result(&value, "", format, w.out);
        }
    }
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
