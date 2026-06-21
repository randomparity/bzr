use serde::{Deserialize, Serialize};

use crate::cli::{BugAction, UpdateArgs};
use crate::client::BugzillaClient;
use crate::commands::runtime::shared::{merge_set, merge_vec};
use crate::error::Result;
use crate::output::result_types::{
    write_result, ActionResult, BatchFailure, BatchResult, DryRunResult, ResourceKind,
};
use crate::output::writers::Writers;
use crate::types::{IdListUpdate, OutputFormat, StringListUpdate, UpdateBugParams};

const FLAG_KEYWORDS_ADD: &str = "--keywords-add";
const FLAG_KEYWORDS_REMOVE: &str = "--keywords-remove";
const FLAG_CC_ADD: &str = "--cc-add";
const FLAG_CC_REMOVE: &str = "--cc-remove";
const FLAG_GROUPS_ADD: &str = "--groups-add";
const FLAG_GROUPS_REMOVE: &str = "--groups-remove";
const FLAG_SEE_ALSO_ADD: &str = "--see-also-add";
const FLAG_SEE_ALSO_REMOVE: &str = "--see-also-remove";

const COMMENT_SUFFIX: &str = " (with comment)";

fn comment_suffix(present: bool) -> &'static str {
    if present {
        COMMENT_SUFFIX
    } else {
        ""
    }
}

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

/// Build an `IdListUpdate` from the raw `--*-add` / `--*-remove` ID lists.
fn id_list_update(add: &[u64], remove: &[u64]) -> IdListUpdate {
    IdListUpdate {
        add: add.to_vec(),
        remove: remove.to_vec(),
    }
}

/// Build a `StringListUpdate`, validating each side via [`clean_string_list`].
/// `add_flag` / `remove_flag` name the originating CLI flags for error context.
fn string_list_update(
    add_flag: &str,
    add: &[String],
    remove_flag: &str,
    remove: &[String],
) -> Result<StringListUpdate> {
    Ok(StringListUpdate {
        add: clean_string_list(add_flag, add)?,
        remove: clean_string_list(remove_flag, remove)?,
    })
}

pub(super) fn resolve_comment(
    comment: Option<&str>,
    comment_file: Option<&std::path::Path>,
    comment_private: bool,
) -> Result<Option<crate::types::CommentUpdate>> {
    let body = crate::commands::runtime::shared::materialize_body_source(
        crate::commands::runtime::shared::classify_body_source(
            comment,
            comment_file,
            "--comment",
            "--comment-file",
        )?,
        "--comment-file",
    )?;
    if body.is_none() && comment_private {
        return Err(crate::error::BzrError::InputValidation(
            "--comment-private requires --comment or --comment-file".into(),
        ));
    }
    let Some(text) = body else {
        return Ok(None);
    };
    if text.trim().is_empty() {
        return Err(crate::error::BzrError::InputValidation(
            "empty comment, aborting".into(),
        ));
    }
    Ok(Some(crate::types::CommentUpdate {
        body: text,
        is_private: comment_private,
    }))
}

pub(super) fn validate_action(action: &BugAction) -> Result<()> {
    match action {
        BugAction::Update(args) => validate_args(args),
        _ => Ok(()),
    }
}

/// Reject `--alias` combined with multiple IDs (Bugzilla allows alias updates
/// for a single bug only).
fn validate_args(args: &UpdateArgs) -> Result<()> {
    if args.dupe_of.is_some() && (args.status.is_some() || args.resolution.is_some()) {
        return Err(crate::error::BzrError::InputValidation(
            "--dupe-of cannot be combined with --status or --resolution".into(),
        ));
    }
    if args.alias.is_some() && args.ids.len() > 1 {
        return Err(crate::error::BzrError::InputValidation(
            "--alias can only be used when updating one bug".into(),
        ));
    }
    Ok(())
}

fn build_update_params(args: &UpdateArgs) -> Result<(Vec<u64>, UpdateBugParams)> {
    validate_args(args)?;

    let UpdateArgs {
        from_json: _,
        ids,
        status,
        resolution,
        dupe_of,
        alias,
        deadline,
        estimated_time,
        remaining_time,
        work_time,
        reset_assigned_to,
        reset_qa_contact,
        assignee,
        priority,
        severity,
        summary,
        whiteboard,
        url,
        target_milestone,
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
        comment,
        comment_file,
        comment_private,
        expect_unchanged_since: _,
    } = args;

    let flags = crate::commands::runtime::flags::parse_flags(flag)?;
    let deadline = crate::validation::parse_optional_date_only(deadline.as_deref(), "--deadline")?;
    let params = UpdateBugParams {
        status: status.clone(),
        resolution: resolution.clone(),
        dupe_of: *dupe_of,
        alias: alias.clone(),
        deadline,
        estimated_time: *estimated_time,
        remaining_time: *remaining_time,
        work_time: *work_time,
        reset_assigned_to: *reset_assigned_to,
        reset_qa_contact: *reset_qa_contact,
        assigned_to: assignee.clone(),
        priority: priority.clone(),
        severity: severity.clone(),
        summary: summary.clone(),
        whiteboard: whiteboard.clone(),
        url: url.clone(),
        target_milestone: target_milestone.clone(),
        flags,
        blocks: id_list_update(blocks_add, blocks_remove),
        depends_on: id_list_update(depends_on_add, depends_on_remove),
        keywords: string_list_update(
            FLAG_KEYWORDS_ADD,
            keywords_add,
            FLAG_KEYWORDS_REMOVE,
            keywords_remove,
        )?,
        cc: string_list_update(FLAG_CC_ADD, cc_add, FLAG_CC_REMOVE, cc_remove)?,
        groups: string_list_update(
            FLAG_GROUPS_ADD,
            groups_add,
            FLAG_GROUPS_REMOVE,
            groups_remove,
        )?,
        see_also: string_list_update(
            FLAG_SEE_ALSO_ADD,
            see_also_add,
            FLAG_SEE_ALSO_REMOVE,
            see_also_remove,
        )?,
        comment: resolve_comment(
            comment.as_deref(),
            comment_file.as_deref(),
            *comment_private,
        )?,
        comment_is_private: std::collections::HashMap::new(),
    };
    if params.is_empty() {
        return Err(crate::error::BzrError::InputValidation(
            "no fields to update; specify at least one field to change".into(),
        ));
    }
    Ok((ids.clone(), params))
}

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

#[derive(Debug)]
enum JsonUpdateInput {
    One(Box<JsonUpdateBug>),
    Many(Vec<JsonUpdateBug>),
}

#[derive(Debug, Serialize)]
struct JsonUpdateRequest {
    id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    expect_unchanged_since: Option<String>,
    #[serde(flatten)]
    params: UpdateBugParams,
}

fn read_from_json(arg: &str) -> Result<String> {
    if arg == "-" {
        crate::commands::runtime::shared::read_stdin_to_string()
    } else {
        crate::commands::runtime::shared::read_file_with_context(
            std::path::Path::new(arg),
            "--from-json",
        )
    }
}

fn parse_json_updates(raw: &str) -> Result<JsonUpdateInput> {
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|e| {
        crate::error::BzrError::InputValidation(format!("--from-json: invalid JSON: {e}"))
    })?;
    match value {
        serde_json::Value::Array(items) => {
            let entries = items
                .into_iter()
                .enumerate()
                .map(|(i, v)| {
                    serde_json::from_value(v).map_err(|e| {
                        crate::error::BzrError::InputValidation(format!(
                            "--from-json item {i}: {e}"
                        ))
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(JsonUpdateInput::Many(entries))
        }
        serde_json::Value::Object(_) => {
            let one = serde_json::from_value(value).map_err(|e| {
                crate::error::BzrError::InputValidation(format!("--from-json: {e}"))
            })?;
            Ok(JsonUpdateInput::One(Box::new(one)))
        }
        _ => Err(crate::error::BzrError::InputValidation(
            "--from-json expects a JSON object or an array of objects".into(),
        )),
    }
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

fn update_args_from_json(entry: JsonUpdateBug, ids: Vec<u64>) -> UpdateArgs {
    UpdateArgs {
        from_json: None,
        ids,
        status: entry.status,
        resolution: entry.resolution,
        dupe_of: entry.dupe_of,
        alias: entry.alias,
        deadline: entry.deadline,
        estimated_time: entry.estimated_time,
        remaining_time: entry.remaining_time,
        work_time: entry.work_time,
        reset_assigned_to: entry.reset_assigned_to.unwrap_or(false),
        reset_qa_contact: entry.reset_qa_contact.unwrap_or(false),
        assignee: entry.assignee,
        priority: entry.priority,
        severity: entry.severity,
        summary: entry.summary,
        whiteboard: entry.whiteboard,
        url: entry.url,
        target_milestone: entry.target_milestone,
        comment: entry.comment,
        comment_file: entry.comment_file,
        comment_private: entry.comment_private.unwrap_or(false),
        flag: entry.flags,
        blocks_add: entry.blocks_add,
        blocks_remove: entry.blocks_remove,
        depends_on_add: entry.depends_on_add,
        depends_on_remove: entry.depends_on_remove,
        keywords_add: entry.keywords_add,
        keywords_remove: entry.keywords_remove,
        cc_add: entry.cc_add,
        cc_remove: entry.cc_remove,
        groups_add: entry.groups_add,
        groups_remove: entry.groups_remove,
        see_also_add: entry.see_also_add,
        see_also_remove: entry.see_also_remove,
        expect_unchanged_since: entry.expect_unchanged_since,
    }
}

fn build_from_json(
    entry: JsonUpdateBug,
    args: &UpdateArgs,
    ids: Vec<u64>,
) -> Result<(Vec<u64>, UpdateBugParams, Option<String>)> {
    reject_json_comment_file_stdin(&entry)?;
    let entry = overlay_cli(entry, args);
    let update_args = update_args_from_json(entry, ids);
    let expected = update_args.expect_unchanged_since.clone();
    let (ids, params) = build_update_params(&update_args)?;
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
    let (_ids, params, expect_unchanged_since) = build_from_json(entry, args, vec![id])?;
    Ok(JsonUpdateRequest {
        id,
        expect_unchanged_since,
        params,
    })
}

async fn update_single(
    client: &BugzillaClient,
    id: u64,
    params: &UpdateBugParams,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    client.update_bug(id, params).await?;
    match format {
        OutputFormat::Json | OutputFormat::Ndjson => {
            write_result(
                &ActionResult::updated(id, ResourceKind::Bug),
                "",
                format,
                w.out,
            );
        }
        OutputFormat::Table => {
            let suffix = comment_suffix(params.comment.is_some());
            let _ = writeln!(w.out, "Updated bug #{id}{suffix}");
        }
    }
    Ok(())
}

fn write_batch_result(
    batch: &BatchResult,
    format: OutputFormat,
    with_comment: bool,
    w: &mut Writers<'_>,
) {
    match format {
        OutputFormat::Json | OutputFormat::Ndjson => {
            write_result(batch, "", format, w.out);
        }
        OutputFormat::Table => {
            if !batch.succeeded.is_empty() {
                let ids_str: Vec<String> =
                    batch.succeeded.iter().map(|id| format!("#{id}")).collect();
                let suffix = comment_suffix(with_comment);
                let _ = writeln!(w.out, "Updated bugs: {}{suffix}", ids_str.join(", "));
            }
            for f in &batch.failed {
                let _ = writeln!(w.err, "Failed to update bug #{}: {}", f.id, f.error);
            }
        }
    }
}

/// The shared exit-11 gate for batch mutations: returns `BatchPartialFailure`
/// (exit 11) when any element failed, else `Ok(())`. Used by batch `bug update`
/// and batch `bug create --from-json`.
pub(super) fn ensure_batch_complete(succeeded: usize, failed: usize) -> Result<()> {
    if failed > 0 {
        Err(crate::error::BzrError::BatchPartialFailure { succeeded, failed })
    } else {
        Ok(())
    }
}

async fn update_batch(
    client: &BugzillaClient,
    ids: &[u64],
    params: &UpdateBugParams,
    format: OutputFormat,
    w: &mut Writers<'_>,
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
    write_batch_result(&batch, format, params.comment.is_some(), w);
    ensure_batch_complete(batch.succeeded.len(), batch.failed.len())
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
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    if crate::commands::runtime::dry_run::enabled() {
        write_json_array_dry_run(requests, format, w);
        return Ok(());
    }
    if !confirm_batch(requests.len(), w)? {
        let _ = writeln!(w.err, "Aborted; no changes made.");
        return Ok(());
    }

    let mut succeeded = Vec::new();
    let mut failed = Vec::new();
    for request in requests {
        if let Some(expected) = request.expect_unchanged_since.as_deref() {
            if let Err(e) = ensure_unchanged_since(client, &[request.id], expected).await {
                failed.push(BatchFailure {
                    id: request.id,
                    error: e.to_string(),
                });
                continue;
            }
        }
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
    write_batch_result(&batch, format, with_comment, w);
    ensure_batch_complete(batch.succeeded.len(), batch.failed.len())
}

async fn handle_from_json(
    client: &BugzillaClient,
    args: &UpdateArgs,
    arg: &str,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    if arg == "-" && cli_comment_uses_stdin(args) {
        reject_cli_stdin_comment_source(args, arg, false)?;
    }
    let raw = read_from_json(arg)?;
    match parse_json_updates(&raw)? {
        JsonUpdateInput::One(entry) => {
            reject_cli_stdin_comment_source(args, arg, false)?;
            let ids = object_ids(&entry, args)?;
            let (ids, params, expect_unchanged_since) = build_from_json(*entry, args, ids)?;
            apply_checked(
                client,
                ApplyRequest {
                    ids,
                    params,
                    expect_unchanged_since: expect_unchanged_since.as_deref(),
                },
                format,
                w,
            )
            .await
        }
        JsonUpdateInput::Many(entries) => {
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
            update_many_from_json(client, &requests, format, w).await
        }
    }
}

pub(super) async fn handle(
    client: &BugzillaClient,
    args: &UpdateArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    if let Some(arg) = args.from_json.as_deref() {
        return handle_from_json(client, args, arg, format, w).await;
    }
    let (ids, params) = build_update_params(args)?;
    apply_checked(
        client,
        ApplyRequest {
            ids,
            params,
            expect_unchanged_since: args.expect_unchanged_since.as_deref(),
        },
        format,
        w,
    )
    .await
}

/// Emit the would-be update without writing: the affected IDs and the payload
/// that would be sent, marked `"action":"dry-run"`. Shared by `bug update` and
/// the convenience verbs.
fn write_update_dry_run(
    ids: &[u64],
    params: &UpdateBugParams,
    format: OutputFormat,
    w: &mut Writers<'_>,
) {
    let ids_str: Vec<String> = ids.iter().map(|id| format!("#{id}")).collect();
    let suffix = comment_suffix(params.comment.is_some());
    write_result(
        &DryRunResult::new(ResourceKind::Bug, ids, params),
        &format!(
            "Dry run: would update bug(s) {}{suffix} (no changes made)",
            ids_str.join(", ")
        ),
        format,
        w.out,
    );
}

/// Apply an already-built `UpdateBugParams` to one or more bug IDs, dispatching
/// to the single- or batch-update path. Shared by `bug update` and the
/// convenience verbs (`resolve`/`close`/`reopen`/`dup`). Under `--dry-run`,
/// previews the change without calling the write API.
pub(super) async fn apply(
    client: &BugzillaClient,
    ids: Vec<u64>,
    params: UpdateBugParams,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    if crate::commands::runtime::dry_run::enabled() {
        write_update_dry_run(&ids, &params, format, w);
        return Ok(());
    }
    if !confirm_batch(ids.len(), w)? {
        let _ = writeln!(w.err, "Aborted; no changes made.");
        return Ok(());
    }
    if ids.len() == 1 {
        update_single(client, ids[0], &params, format, w).await
    } else {
        update_batch(client, &ids, &params, format, w).await
    }
}

pub(super) struct ApplyRequest<'a> {
    pub ids: Vec<u64>,
    pub params: UpdateBugParams,
    pub expect_unchanged_since: Option<&'a str>,
}

/// Apply an update after running the optional optimistic-concurrency guard.
///
/// The guard is skipped under `--dry-run`, which performs no write and whose
/// preview still uses the same payload validation path.
pub(super) async fn apply_checked(
    client: &BugzillaClient,
    request: ApplyRequest<'_>,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    if let Some(expected) = request.expect_unchanged_since {
        if !crate::commands::runtime::dry_run::enabled() {
            ensure_unchanged_since(client, &request.ids, expected).await?;
        }
    }
    apply(client, request.ids, request.params, format, w).await
}

/// Optimistic-concurrency guard for `--expect-unchanged-since`: refuse the
/// update if any target bug's current `last_change_time` differs from
/// `expected`. The check is client-side — Bugzilla's REST `Bug.update` has no
/// atomic compare-and-set — so a narrow window remains between this re-read and
/// the write. All IDs are checked before any write, so a batch is
/// all-or-nothing on collision.
async fn ensure_unchanged_since(
    client: &BugzillaClient,
    ids: &[u64],
    expected: &str,
) -> Result<()> {
    let expected_key = crate::validation::timestamp_compare_key(expected).ok_or_else(|| {
        crate::error::BzrError::InputValidation(format!(
            "--expect-unchanged-since: '{expected}' is not a recognized UTC timestamp; \
             pass the last_change_time value from a preceding `bug view`"
        ))
    })?;
    for &id in ids {
        let bug = client
            .get_bug(&id.to_string(), Some("id,last_change_time"), None)
            .await?;
        let actual = bug.last_change_time.ok_or_else(|| {
            crate::error::BzrError::DataIntegrity(format!(
                "bug {id} returned no last_change_time; cannot verify --expect-unchanged-since"
            ))
        })?;
        let actual_key = crate::validation::timestamp_compare_key(&actual).ok_or_else(|| {
            crate::error::BzrError::DataIntegrity(format!(
                "bug {id} returned an unrecognized last_change_time '{actual}'"
            ))
        })?;
        if actual_key != expected_key {
            return Err(crate::error::BzrError::MidAirCollision {
                id,
                expected: expected.to_string(),
                actual,
            });
        }
    }
    Ok(())
}

/// Prompt for confirmation before a large batch mutation, wiring the real
/// stdin/TTY into the testable [`crate::commands::runtime::confirm`] primitives. The
/// `should_prompt` gate is checked first, so stdin is locked only when a prompt
/// is actually shown. Returns whether to proceed.
fn confirm_batch(count: usize, w: &mut Writers<'_>) -> Result<bool> {
    use std::io::IsTerminal;
    let is_tty = std::io::stdin().is_terminal();
    if !crate::commands::runtime::confirm::should_prompt(
        count,
        crate::commands::runtime::confirm::yes(),
        is_tty,
    ) {
        return Ok(true);
    }
    let stdin = std::io::stdin();
    crate::commands::runtime::confirm::read_yes_no(&mut stdin.lock(), w.err, count)
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
