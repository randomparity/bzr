use serde::{Deserialize, Serialize};

use crate::cli::UpdateArgs;
use crate::client::BugzillaClient;
use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::from_json::JsonOneOrMany;
use crate::commands::runtime::shared::{merge_set, merge_vec};
use crate::error::Result;
use crate::output::result_types::{
    write_result, BatchFailure, BatchResult, DryRunResult, ResourceKind,
};
use crate::output::writers::Writers;
use crate::types::{OutputFormat, UpdateBugParams};

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonUpdateBug {
    id: Option<u64>,
    status: Option<String>,
    resolution: Option<String>,
    dupe_of: Option<u64>,
    alias: Option<String>,
    deadline: Option<String>,
    estimated_time: Option<f64>,
    remaining_time: Option<f64>,
    work_time: Option<f64>,
    reset_assigned_to: Option<bool>,
    reset_qa_contact: Option<bool>,
    assignee: Option<String>,
    priority: Option<String>,
    severity: Option<String>,
    summary: Option<String>,
    whiteboard: Option<String>,
    url: Option<String>,
    target_milestone: Option<String>,
    comment: Option<String>,
    comment_file: Option<std::path::PathBuf>,
    comment_private: Option<bool>,
    #[serde(default)]
    flags: Vec<String>,
    #[serde(default)]
    blocks_add: Vec<u64>,
    #[serde(default)]
    blocks_remove: Vec<u64>,
    #[serde(default)]
    depends_on_add: Vec<u64>,
    #[serde(default)]
    depends_on_remove: Vec<u64>,
    #[serde(default)]
    keywords_add: Vec<String>,
    #[serde(default)]
    keywords_remove: Vec<String>,
    #[serde(default)]
    cc_add: Vec<String>,
    #[serde(default)]
    cc_remove: Vec<String>,
    #[serde(default)]
    groups_add: Vec<String>,
    #[serde(default)]
    groups_remove: Vec<String>,
    #[serde(default)]
    see_also_add: Vec<String>,
    #[serde(default)]
    see_also_remove: Vec<String>,
    expect_unchanged_since: Option<String>,
}

#[derive(Debug, Serialize)]
struct JsonUpdateRequest {
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    expect_unchanged_since: Option<String>,
    #[serde(flatten)]
    params: UpdateBugParams,
}

fn merge_copy<T: Copy>(target: &mut Option<T>, value: Option<T>) {
    if let Some(value) = value {
        *target = Some(value);
    }
}

fn merge_bool_true(target: &mut Option<bool>, value: bool) {
    if value {
        *target = Some(true);
    }
}

fn merge_path(target: &mut Option<std::path::PathBuf>, value: Option<&std::path::Path>) {
    if let Some(value) = value {
        *target = Some(value.to_path_buf());
    }
}

fn merge_vec_u64(target: &mut Vec<u64>, value: &[u64]) {
    if !value.is_empty() {
        *target = value.to_vec();
    }
}

fn overlay_cli(mut json: JsonUpdateBug, args: &UpdateArgs) -> JsonUpdateBug {
    merge_set(&mut json.status, args.status.as_deref());
    merge_set(&mut json.resolution, args.resolution.as_deref());
    merge_copy(&mut json.dupe_of, args.dupe_of);
    merge_set(&mut json.alias, args.alias.as_deref());
    merge_set(&mut json.deadline, args.deadline.as_deref());
    merge_copy(&mut json.estimated_time, args.estimated_time);
    merge_copy(&mut json.remaining_time, args.remaining_time);
    merge_copy(&mut json.work_time, args.work_time);
    merge_bool_true(&mut json.reset_assigned_to, args.reset_assigned_to);
    merge_bool_true(&mut json.reset_qa_contact, args.reset_qa_contact);
    merge_set(&mut json.assignee, args.assignee.as_deref());
    merge_set(&mut json.priority, args.priority.as_deref());
    merge_set(&mut json.severity, args.severity.as_deref());
    merge_set(&mut json.summary, args.summary.as_deref());
    merge_set(&mut json.whiteboard, args.whiteboard.as_deref());
    merge_set(&mut json.url, args.url.as_deref());
    merge_set(&mut json.target_milestone, args.target_milestone.as_deref());
    if let Some(comment) = args.comment.as_deref() {
        json.comment = Some(comment.to_string());
        json.comment_file = None;
    }
    if args.comment_file.is_some() {
        json.comment = None;
        merge_path(&mut json.comment_file, args.comment_file.as_deref());
    }
    merge_bool_true(&mut json.comment_private, args.comment_private);
    merge_vec(&mut json.flags, &args.flag);
    merge_vec_u64(&mut json.blocks_add, &args.blocks_add);
    merge_vec_u64(&mut json.blocks_remove, &args.blocks_remove);
    merge_vec_u64(&mut json.depends_on_add, &args.depends_on_add);
    merge_vec_u64(&mut json.depends_on_remove, &args.depends_on_remove);
    merge_vec(&mut json.keywords_add, &args.keywords_add);
    merge_vec(&mut json.keywords_remove, &args.keywords_remove);
    merge_vec(&mut json.cc_add, &args.cc_add);
    merge_vec(&mut json.cc_remove, &args.cc_remove);
    merge_vec(&mut json.groups_add, &args.groups_add);
    merge_vec(&mut json.groups_remove, &args.groups_remove);
    merge_vec(&mut json.see_also_add, &args.see_also_add);
    merge_vec(&mut json.see_also_remove, &args.see_also_remove);
    merge_set(
        &mut json.expect_unchanged_since,
        args.expect_unchanged_since.as_deref(),
    );
    json
}

fn reject_json_comment_file_stdin(entry: &JsonUpdateBug) -> Result<()> {
    if entry.comment_file.as_deref() == Some(std::path::Path::new("-")) {
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

fn reject_cli_stdin_comment_source(
    args: &UpdateArgs,
    from_json_arg: &str,
    is_array: bool,
) -> Result<()> {
    if !cli_comment_uses_stdin(args) {
        return Ok(());
    }
    if from_json_arg == "-" {
        return Err(crate::error::BzrError::InputValidation(
            "--from-json - cannot be combined with --comment - or --comment-file -".into(),
        ));
    }
    if is_array {
        return Err(crate::error::BzrError::InputValidation(
            "--from-json array input cannot combine with --comment - or --comment-file -; \
             put per-entry comments in JSON"
                .into(),
        ));
    }
    Ok(())
}

fn object_ids(entry: &JsonUpdateBug, args: &UpdateArgs) -> Result<Vec<u64>> {
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

fn update_input_from_json<'a>(
    entry: &'a JsonUpdateBug,
    ids: &'a [u64],
) -> super::update::BugUpdateInput<'a> {
    super::update::BugUpdateInput {
        ids,
        status: entry.status.as_deref(),
        resolution: entry.resolution.as_deref(),
        dupe_of: entry.dupe_of,
        alias: entry.alias.as_deref(),
        deadline: entry.deadline.as_deref(),
        estimated_time: entry.estimated_time,
        remaining_time: entry.remaining_time,
        work_time: entry.work_time,
        reset_assigned_to: entry.reset_assigned_to.unwrap_or(false),
        reset_qa_contact: entry.reset_qa_contact.unwrap_or(false),
        assignee: entry.assignee.as_deref(),
        priority: entry.priority.as_deref(),
        severity: entry.severity.as_deref(),
        summary: entry.summary.as_deref(),
        whiteboard: entry.whiteboard.as_deref(),
        url: entry.url.as_deref(),
        target_milestone: entry.target_milestone.as_deref(),
        comment: entry.comment.as_deref(),
        comment_file: entry.comment_file.as_deref(),
        comment_private: entry.comment_private.unwrap_or(false),
        flags: &entry.flags,
        blocks_add: &entry.blocks_add,
        blocks_remove: &entry.blocks_remove,
        depends_on_add: &entry.depends_on_add,
        depends_on_remove: &entry.depends_on_remove,
        keywords_add: &entry.keywords_add,
        keywords_remove: &entry.keywords_remove,
        cc_add: &entry.cc_add,
        cc_remove: &entry.cc_remove,
        groups_add: &entry.groups_add,
        groups_remove: &entry.groups_remove,
        see_also_add: &entry.see_also_add,
        see_also_remove: &entry.see_also_remove,
    }
}

fn build_from_json(
    entry: JsonUpdateBug,
    args: &UpdateArgs,
    ids: &[u64],
) -> Result<(Vec<u64>, UpdateBugParams, Option<String>)> {
    reject_json_comment_file_stdin(&entry)?;
    let entry = overlay_cli(entry, args);
    let expected = entry.expect_unchanged_since.clone();
    let input = update_input_from_json(&entry, ids);
    let (ids, params) = super::update::build_update_params_from_input(&input)?;
    Ok((ids, params, expected))
}

fn build_array_request(
    entry: JsonUpdateBug,
    args: &UpdateArgs,
    index: usize,
) -> Result<JsonUpdateRequest> {
    let id = entry.id.ok_or_else(|| {
        crate::error::BzrError::InputValidation(format!("--from-json item {index}: id is required"))
    })?;
    let (_ids, params, expect_unchanged_since) = build_from_json(entry, args, &[id])?;
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
    client: &BugzillaClient,
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

    let preflight_failures = preflight_expect_unchanged_since(client, requests).await;
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
    }

    let batch = BatchResult::new(succeeded, failed);
    let with_comment = requests
        .iter()
        .any(|request| request.params.comment.is_some());
    super::update::write_batch_result(&batch, format, with_comment, w);
    super::update::ensure_batch_complete(batch.succeeded.len(), batch.failed.len())
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
    client: &BugzillaClient,
    args: &UpdateArgs,
    arg: &str,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    if arg == "-" && cli_comment_uses_stdin(args) {
        reject_cli_stdin_comment_source(args, arg, false)?;
    }
    match crate::commands::runtime::from_json::read_one_or_many::<JsonUpdateBug>(arg)? {
        JsonOneOrMany::One(entry) => {
            reject_cli_stdin_comment_source(args, arg, false)?;
            let ids = object_ids(&entry, args)?;
            let (ids, params, expect_unchanged_since) = build_from_json(*entry, args, &ids)?;
            super::update::apply_checked(
                client,
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
            reject_cli_stdin_comment_source(args, arg, true)?;
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
            update_many_from_json(client, &requests, ctx, w).await
        }
    }
}

#[cfg(test)]
#[path = "update_json_tests.rs"]
mod tests;
