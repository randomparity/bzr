use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::{self, ActionResult, BatchFailure, BatchResult, ResourceKind};
use crate::types::{IdListUpdate, OutputFormat, StringListUpdate, UpdateBugParams};

fn clean_string_list(field: &str, values: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::with_capacity(values.len());
    for raw in values {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(crate::error::BzrError::InputValidation(format!(
                "{field}: list value cannot be empty or whitespace-only"
            )));
        }
        out.push(trimmed.to_string());
    }
    Ok(out)
}

fn build_update_params(action: &BugAction) -> Result<(Vec<u64>, UpdateBugParams)> {
    let BugAction::Update {
        ids,
        status,
        resolution,
        assignee,
        priority,
        severity,
        summary,
        whiteboard,
        flag,
        blocks_add,
        blocks_remove,
        depends_on_add,
        depends_on_remove,
        keywords_add,
        keywords_remove,
        cc_add,
        cc_remove,
        groups_add,
        groups_remove,
        see_also_add,
        see_also_remove,
    } = action
    else {
        unreachable!()
    };

    let flags = crate::commands::flags::parse_flags(flag)?;
    let params = UpdateBugParams {
        status: status.clone(),
        resolution: resolution.clone(),
        assigned_to: assignee.clone(),
        priority: priority.clone(),
        severity: severity.clone(),
        summary: summary.clone(),
        whiteboard: whiteboard.clone(),
        flags,
        blocks: IdListUpdate {
            add: blocks_add.clone(),
            remove: blocks_remove.clone(),
        },
        depends_on: IdListUpdate {
            add: depends_on_add.clone(),
            remove: depends_on_remove.clone(),
        },
        keywords: StringListUpdate {
            add: clean_string_list("keywords-add", keywords_add)?,
            remove: clean_string_list("keywords-remove", keywords_remove)?,
        },
        cc: StringListUpdate {
            add: clean_string_list("cc-add", cc_add)?,
            remove: clean_string_list("cc-remove", cc_remove)?,
        },
        groups: StringListUpdate {
            add: clean_string_list("groups-add", groups_add)?,
            remove: clean_string_list("groups-remove", groups_remove)?,
        },
        see_also: StringListUpdate {
            add: clean_string_list("see-also-add", see_also_add)?,
            remove: clean_string_list("see-also-remove", see_also_remove)?,
        },
    };
    Ok((ids.clone(), params))
}

async fn update_single(
    client: &BugzillaClient,
    id: u64,
    params: &UpdateBugParams,
    format: OutputFormat,
) -> Result<()> {
    client.update_bug(id, params).await?;
    output::print_result(
        &ActionResult::updated(id, ResourceKind::Bug),
        &format!("Updated bug #{id}"),
        format,
    );
    Ok(())
}

fn print_batch_result(batch: &BatchResult, format: OutputFormat) {
    use std::io::Write;
    match format {
        OutputFormat::Json => {
            output::print_result(batch, "", format);
        }
        OutputFormat::Table => {
            if !batch.succeeded.is_empty() {
                let ids_str: Vec<String> =
                    batch.succeeded.iter().map(|id| format!("#{id}")).collect();
                let _ = writeln!(std::io::stdout(), "Updated bugs: {}", ids_str.join(", "));
            }
            for f in &batch.failed {
                let _ = writeln!(
                    std::io::stderr(),
                    "Failed to update bug #{}: {}",
                    f.id,
                    f.error
                );
            }
        }
    }
}

async fn update_batch(
    client: &BugzillaClient,
    ids: &[u64],
    params: &UpdateBugParams,
    format: OutputFormat,
) -> Result<()> {
    let mut succeeded = Vec::new();
    let mut failed = Vec::new();
    for &id in ids {
        match client.update_bug(id, params).await {
            Ok(()) => succeeded.push(id),
            Err(e) => failed.push(BatchFailure {
                id,
                error: e.to_string(),
            }),
        }
    }
    let batch = BatchResult::new(succeeded, failed);
    print_batch_result(&batch, format);
    if !batch.failed.is_empty() {
        return Err(crate::error::BzrError::BatchPartialFailure {
            succeeded: batch.succeeded.len(),
            failed: batch.failed.len(),
        });
    }
    Ok(())
}

pub(super) async fn handle(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let (ids, params) = build_update_params(action)?;
    if ids.len() == 1 {
        update_single(client, ids[0], &params, format).await
    } else {
        update_batch(client, &ids, &params, format).await
    }
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
