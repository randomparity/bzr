use serde::Serialize;

use crate::cli::UpdateArgs;
use crate::client::BugzillaClient;
use crate::commands::runtime::input::from_json::JsonOneOrMany;
use crate::commands::runtime::invocation::CommandContext;
use crate::error::Result;
use crate::output::result_types::{
    write_result, BatchFailure, BatchResult, DryRunResult, ResourceKind,
};
use crate::output::writers::Writers;
use crate::types::bug::UpdateBugParams;
use crate::types::output::OutputFormat;

#[derive(Debug, Serialize)]
struct JsonUpdateRequest {
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    expect_unchanged_since: Option<String>,
    #[serde(flatten)]
    params: UpdateBugParams,
}

fn reject_json_comment_file_stdin(
    entry: &super::update::BugUpdateDraft,
    args: &UpdateArgs,
) -> Result<()> {
    let cli_comment_file_stdin = args.comment_file.as_deref() == Some(std::path::Path::new("-"));
    if entry.comment_file.as_deref() == Some(std::path::Path::new("-")) && !cli_comment_file_stdin {
        return Err(crate::error::BzrError::InputValidation(
            "--from-json comment_file cannot read from stdin; use comment text in JSON instead"
                .into(),
        ));
    }
    Ok(())
}

fn cli_comment_uses_stdin(args: &UpdateArgs) -> bool {
    args.comment.as_deref() == Some("-")
        || args.comment_file.as_deref() == Some(std::path::Path::new("-"))
}

#[derive(Clone, Copy)]
enum JsonUpdateInputSource {
    Stdin,
    FileObject,
    FileArray,
}

fn reject_cli_stdin_comment_source(args: &UpdateArgs, source: JsonUpdateInputSource) -> Result<()> {
    if !cli_comment_uses_stdin(args) {
        return Ok(());
    }
    match source {
        JsonUpdateInputSource::Stdin => Err(crate::error::BzrError::InputValidation(
            "--from-json - cannot be combined with --comment - or --comment-file -".into(),
        )),
        JsonUpdateInputSource::FileArray => Err(crate::error::BzrError::InputValidation(
            "--from-json array input cannot combine with --comment - or --comment-file -; \
             put per-entry comments in JSON"
                .into(),
        )),
        JsonUpdateInputSource::FileObject => Ok(()),
    }
}

fn read_validated_update_input(
    args: &UpdateArgs,
    arg: &str,
) -> Result<JsonOneOrMany<super::update::BugUpdateDraft>> {
    if arg == "-" {
        reject_cli_stdin_comment_source(args, JsonUpdateInputSource::Stdin)?;
        return crate::commands::runtime::input::from_json::read_one_or_many(arg);
    }

    let input = crate::commands::runtime::input::from_json::read_one_or_many(arg)?;
    match &input {
        JsonOneOrMany::One(_) => {
            reject_cli_stdin_comment_source(args, JsonUpdateInputSource::FileObject)?;
        }
        JsonOneOrMany::Many(_) => {
            reject_cli_stdin_comment_source(args, JsonUpdateInputSource::FileArray)?;
        }
    }
    Ok(input)
}

fn object_ids(entry: &super::update::BugUpdateDraft, args: &UpdateArgs) -> Result<Vec<u64>> {
    if !args.ids.is_empty() {
        if entry.id.is_some() {
            return Err(crate::error::BzrError::InputValidation(
                "--from-json object cannot combine positional IDs with JSON id".into(),
            ));
        }
        return Ok(args.ids.clone());
    }
    entry.id.map(|id| vec![id]).ok_or_else(|| {
        crate::error::BzrError::InputValidation(
            "--from-json object requires positional IDs or an id field".into(),
        )
    })
}

fn build_from_json(
    mut entry: super::update::BugUpdateDraft,
    args: &UpdateArgs,
    ids: Vec<u64>,
) -> Result<(Vec<u64>, UpdateBugParams, Option<String>)> {
    entry.overlay_cli(args);
    reject_json_comment_file_stdin(&entry, args)?;
    let expected = entry.expect_unchanged_since.clone();
    let (ids, params) = super::update::build_update_params_from_draft(ids, &entry)?;
    Ok((ids, params, expected))
}

fn build_array_request(
    entry: super::update::BugUpdateDraft,
    args: &UpdateArgs,
    index: usize,
) -> Result<JsonUpdateRequest> {
    let id = entry.id.ok_or_else(|| {
        crate::error::BzrError::InputValidation(format!("--from-json item {index}: id is required"))
    })?;
    let (_ids, params, expect_unchanged_since) = build_from_json(entry, args, vec![id])?;
    Ok(JsonUpdateRequest {
        id,
        expect_unchanged_since,
        params,
    })
}

fn write_json_array_dry_run(
    requests: &[JsonUpdateRequest],
    format: OutputFormat,
    w: &mut Writers<'_>,
) {
    let ids: Vec<u64> = requests.iter().map(|request| request.id).collect();
    let ids_str: Vec<String> = ids.iter().map(|id| format!("#{id}")).collect();
    write_result(
        &DryRunResult::new(ResourceKind::Bug, &ids, &requests),
        &format!(
            "Dry run: would update bug(s) {} (no changes made)",
            ids_str.join(", ")
        ),
        format,
        w.out,
    );
}

async fn update_many_from_json(
    requests: &[JsonUpdateRequest],
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    if ctx.dry_run() {
        write_json_array_dry_run(requests, format, w);
        return Ok(());
    }
    if !super::update::confirm_batch(requests.len(), ctx.assume_yes(), w)? {
        let _ = writeln!(w.err, "Aborted; no changes made.");
        return Ok(());
    }

    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let preflight_failures = preflight_expect_unchanged_since(&client, requests).await;
    if !preflight_failures.is_empty() {
        let batch = BatchResult::new(Vec::new(), preflight_failures);
        let with_comment = requests
            .iter()
            .any(|request| request.params.comment.is_some());
        super::update::write_batch_result(&batch, format, with_comment, w);
        return super::update::ensure_batch_complete(batch.succeeded.len(), batch.failed.len());
    }

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();
    for request in requests {
        match client.update_bug(request.id, &request.params).await {
            Ok(()) => succeeded.push(request.id),
            Err(e) => failed.push(BatchFailure {
                id: request.id,
                error: e.to_string(),
            }),
        }
        crate::output::progress::batch_event(
            ctx.progress(),
            w.err,
            &crate::output::progress::BatchProgress {
                n: succeeded.len() + failed.len(),
                total: requests.len(),
                ok: succeeded.len(),
                failed: failed.len(),
            },
        );
    }

    let batch = BatchResult::new(succeeded, failed);
    let with_comment = requests
        .iter()
        .any(|request| request.params.comment.is_some());
    super::update::write_batch_result(&batch, format, with_comment, w);
    let result = super::update::ensure_batch_complete(batch.succeeded.len(), batch.failed.len());
    if result.is_ok() {
        crate::output::progress::done_event(ctx.progress(), w.err, requests.len());
    }
    result
}

async fn preflight_expect_unchanged_since(
    client: &BugzillaClient,
    requests: &[JsonUpdateRequest],
) -> Vec<BatchFailure> {
    let mut failed = Vec::new();
    for request in requests {
        let Some(expected) = request.expect_unchanged_since.as_deref() else {
            continue;
        };
        if let Err(e) = super::update::ensure_unchanged_since(client, &[request.id], expected).await
        {
            failed.push(BatchFailure {
                id: request.id,
                error: e.to_string(),
            });
        }
    }
    failed
}

pub(super) async fn handle(
    args: &UpdateArgs,
    arg: &str,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    match read_validated_update_input(args, arg)? {
        JsonOneOrMany::One(entry) => {
            let ids = object_ids(&entry, args)?;
            let (ids, params, expect_unchanged_since) = build_from_json(*entry, args, ids)?;
            super::update::apply_checked(
                super::update::ApplyRequest {
                    ids,
                    params,
                    expect_unchanged_since: expect_unchanged_since.as_deref(),
                },
                ctx,
                w,
            )
            .await
        }
        JsonOneOrMany::Many(entries) => {
            if !args.ids.is_empty() {
                return Err(crate::error::BzrError::InputValidation(
                    "--from-json array input cannot be combined with positional IDs".into(),
                ));
            }
            if entries.is_empty() {
                return Err(crate::error::BzrError::InputValidation(
                    "--from-json: empty array, nothing to update".into(),
                ));
            }
            let mut requests = Vec::with_capacity(entries.len());
            for (index, entry) in entries.into_iter().enumerate() {
                requests.push(build_array_request(entry, args, index)?);
            }
            update_many_from_json(&requests, ctx, w).await
        }
    }
}

#[cfg(test)]
#[path = "update_json_tests.rs"]
mod tests;
