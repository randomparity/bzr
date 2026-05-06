use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::{BzrError, Result};
use crate::output::{self, print_multi_bug_view, BugViewFailure, MultiBugRow, MultiBugViewResult};
use crate::types::{Bug, OutputFormat};

pub(super) async fn handle(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
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
        view_single(client, &ids[0], inc, exc, format).await
    } else if *permissive {
        view_batch_permissive(client, ids, inc, exc, format).await
    } else {
        view_batch_strict(client, ids, inc, exc, format).await
    }
}

async fn view_single(
    client: &BugzillaClient,
    id: &str,
    include_fields: Option<&str>,
    exclude_fields: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let bug = client.get_bug(id, include_fields, exclude_fields).await?;
    output::print_bug_detail(&bug, format);
    Ok(())
}

/// Multi-ID strict path. On any failure, bail with that error's code.
///
/// Output handling differs by format:
/// - Table: eager-print each detail block as it arrives. Preceding
///   successes have already been written to stdout when the failure
///   propagates.
/// - JSON: collect all bugs in a buffer; emit nothing on failure
///   (concatenated bare JSON objects would be invalid).
async fn view_batch_strict(
    client: &BugzillaClient,
    ids: &[String],
    include_fields: Option<&str>,
    exclude_fields: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Table => {
            for (i, id) in ids.iter().enumerate() {
                let bug = client.get_bug(id, include_fields, exclude_fields).await?;
                if i > 0 {
                    output::write_divider(&mut std::io::stdout());
                }
                output::print_bug_detail(&bug, format);
            }
            Ok(())
        }
        OutputFormat::Json => {
            let mut bugs = Vec::with_capacity(ids.len());
            for id in ids {
                let bug = client.get_bug(id, include_fields, exclude_fields).await?;
                bugs.push(bug);
            }
            let result = MultiBugViewResult {
                bugs,
                failed: Vec::new(),
            };
            output::print_result(&result, "", format);
            Ok(())
        }
    }
}

/// Multi-ID permissive path.
///
/// Per-resource errors (`BzrError::is_bug_get_per_resource()` — i.e.
/// `NotFound` and `Bug.get` codes 100/101/102) are recorded as inline
/// failure rows. Anything else (transport, auth, security, server
/// internal, unrecognized `Api` codes) bails immediately with that
/// error's exit code.
async fn view_batch_permissive(
    client: &BugzillaClient,
    ids: &[String],
    include_fields: Option<&str>,
    exclude_fields: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    match format {
        OutputFormat::Table => {
            let mut rows: Vec<MultiBugRow> = Vec::with_capacity(ids.len());
            for id in ids {
                match client.get_bug(id, include_fields, exclude_fields).await {
                    Ok(bug) => rows.push(MultiBugRow::Ok(Box::new(bug))),
                    Err(e) if e.is_bug_get_per_resource() => {
                        rows.push(MultiBugRow::Failed {
                            id: id.clone(),
                            error: e.to_string(),
                        });
                    }
                    Err(e) => return Err(e),
                }
            }
            print_multi_bug_view(&rows);
        }
        OutputFormat::Json => {
            let mut bugs: Vec<Bug> = Vec::with_capacity(ids.len());
            let mut failed: Vec<BugViewFailure> = Vec::new();
            for id in ids {
                match client.get_bug(id, include_fields, exclude_fields).await {
                    Ok(bug) => bugs.push(bug),
                    Err(e) if e.is_bug_get_per_resource() => {
                        failed.push(BugViewFailure {
                            id: id.clone(),
                            error: e.to_string(),
                        });
                    }
                    Err(e) => return Err(e),
                }
            }
            output::print_result(&MultiBugViewResult { bugs, failed }, "", format);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
