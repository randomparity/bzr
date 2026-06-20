use std::io::IsTerminal;

use serde::Deserialize;

use crate::cli::CreateArgs;
use crate::client::BugzillaClient;
use crate::commands::runtime::editor;
use crate::commands::runtime::shared::{merge_set, merge_vec};
use crate::error::Result;
use crate::output::result_types::{
    write_result, ActionResult, BatchCreateResult, CreateFailure, DryRunResult, ResourceKind,
};
use crate::output::writers::Writers;
use crate::types::{CreateBugParams, OutputFormat};

const SENTINEL: &str = "# ------------------------ >8 ------------------------";

/// CLI-over-template field merge, computed once and shared between
/// the editor preview and the final `CreateBugParams` build.
struct MergedFields {
    product: String,
    component: String,
    version: Option<String>,
    priority: Option<String>,
    severity: Option<String>,
    assigned_to: Option<String>,
    op_sys: Option<String>,
    rep_platform: Option<String>,
    template_description: Option<String>,
}

impl MergedFields {
    fn preview_params(&self) -> CreateBugParams {
        CreateBugParams {
            product: self.product.clone(),
            component: self.component.clone(),
            version: self
                .version
                .clone()
                .unwrap_or_else(|| "unspecified".to_string()),
            priority: self.priority.clone(),
            severity: self.severity.clone(),
            assigned_to: self.assigned_to.clone(),
            op_sys: self.op_sys.clone(),
            rep_platform: self.rep_platform.clone(),
            ..Default::default()
        }
    }
}

/// Parse the post-editor buffer into `(summary, description)`.
fn parse_editor_buffer(raw: &str) -> Result<(String, String)> {
    let mut iter = raw
        .lines()
        .take_while(|l| l.trim_end() != SENTINEL)
        .skip_while(|l| l.trim().is_empty());

    let summary = iter
        .next()
        .map(|l| l.trim().to_string())
        .ok_or_else(|| crate::error::BzrError::InputValidation("empty buffer, aborting".into()))?;

    let body: Vec<&str> = iter.collect();
    let start = body.iter().position(|l| !l.trim().is_empty()).unwrap_or(0);
    let end = body
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(0, |i| i + 1);
    let description = body
        .get(start..end)
        .map(|s| s.join("\n"))
        .unwrap_or_default();

    Ok((summary, description))
}

fn build_editor_template(
    summary_pre_fill: Option<&str>,
    template_description: Option<&str>,
    params: &CreateBugParams,
) -> String {
    let mut buf = String::new();
    buf.push_str(summary_pre_fill.unwrap_or(""));
    buf.push('\n');
    buf.push('\n');
    if let Some(body) = template_description {
        buf.push_str(body);
        if !body.ends_with('\n') {
            buf.push('\n');
        }
        buf.push('\n');
    }
    buf.push_str(SENTINEL);
    buf.push('\n');
    buf.push_str("# Do not modify or remove the line above.\n");
    buf.push_str("# Everything below it will be ignored.\n");
    buf.push_str("#\n");
    let row = |label: &str, val: &str| format!("# {label:<12}{val}\n");
    let unset = "<unset>";
    buf.push_str(&row("Product:", &params.product));
    buf.push_str(&row("Component:", &params.component));
    buf.push_str(&row("Version:", &params.version));
    buf.push_str(&row(
        "Priority:",
        params.priority.as_deref().unwrap_or(unset),
    ));
    buf.push_str(&row(
        "Severity:",
        params.severity.as_deref().unwrap_or(unset),
    ));
    buf.push_str(&row(
        "Assignee:",
        params.assigned_to.as_deref().unwrap_or(unset),
    ));
    buf.push_str(&row("OpSys:", params.op_sys.as_deref().unwrap_or(unset)));
    buf.push_str(&row(
        "Platform:",
        params.rep_platform.as_deref().unwrap_or(unset),
    ));
    buf
}

fn resolve_description(
    description: Option<&str>,
    description_file: Option<&std::path::Path>,
) -> Result<Option<String>> {
    let explicit = crate::commands::runtime::shared::materialize_body_source(
        crate::commands::runtime::shared::classify_body_source(
            description,
            description_file,
            "--description",
            "--description-file",
        )?,
        "--description-file",
    )?;
    if explicit.is_some() {
        return Ok(explicit);
    }
    if !std::io::stdin().is_terminal() {
        let buf = crate::commands::runtime::shared::read_stdin_to_string()?;
        if buf.trim().is_empty() {
            return Err(crate::error::BzrError::InputValidation(
                "no description supplied (piped stdin is empty)".into(),
            ));
        }
        return Ok(Some(buf));
    }
    Ok(None)
}

fn load_template(name: Option<&str>) -> Result<Option<crate::types::BugTemplate>> {
    let Some(name) = name else { return Ok(None) };
    let config = crate::config::Config::load()?;
    let t = config
        .templates
        .get(name)
        .ok_or_else(|| crate::error::BzrError::config(format!("template '{name}' not found")))?;
    Ok(Some(t.clone()))
}

fn run_editor_flow(
    summary_pre_fill: Option<&str>,
    merged: &MergedFields,
) -> Result<(String, String)> {
    let preview = merged.preview_params();
    let template_buf = build_editor_template(
        summary_pre_fill,
        merged.template_description.as_deref(),
        &preview,
    );
    let raw = editor::launch(&template_buf, "bug-create")?;
    parse_editor_buffer(&raw)
}

fn merge_fields(
    args: &CreateArgs,
    tmpl: Option<&crate::types::BugTemplate>,
) -> Result<MergedFields> {
    let CreateArgs {
        product,
        component,
        version,
        priority,
        severity,
        assignee,
        op_sys,
        rep_platform,
        ..
    } = args;
    let resolved_product = product
        .clone()
        .or_else(|| tmpl.and_then(|t| t.product.clone()))
        .ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "--product is required (provide it directly or via a template)".into(),
            )
        })?;
    let resolved_component = component
        .clone()
        .or_else(|| tmpl.and_then(|t| t.component.clone()))
        .ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "--component is required (provide it directly or via a template)".into(),
            )
        })?;
    Ok(MergedFields {
        product: resolved_product,
        component: resolved_component,
        version: version
            .clone()
            .or_else(|| tmpl.and_then(|t| t.version.clone())),
        priority: priority
            .clone()
            .or_else(|| tmpl.and_then(|t| t.priority.clone())),
        severity: severity
            .clone()
            .or_else(|| tmpl.and_then(|t| t.severity.clone())),
        assigned_to: assignee
            .clone()
            .or_else(|| tmpl.and_then(|t| t.assignee.clone())),
        op_sys: op_sys
            .clone()
            .or_else(|| tmpl.and_then(|t| t.op_sys.clone())),
        rep_platform: rep_platform
            .clone()
            .or_else(|| tmpl.and_then(|t| t.rep_platform.clone())),
        template_description: tmpl.and_then(|t| t.description.clone()),
    })
}

/// One bug's worth of structured input for `bug create --from-json`. Keys match
/// the create flag names; `deny_unknown_fields` rejects typos and keeps
/// undesigned `cf_*` custom-field writes (issue #283) out of this path. All
/// fields are optional here — required-field and date validation happen in
/// [`Self::into_params`] so the error messages can name the offending field.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonCreateBug {
    product: Option<String>,
    component: Option<String>,
    summary: Option<String>,
    version: Option<String>,
    description: Option<String>,
    priority: Option<String>,
    severity: Option<String>,
    assignee: Option<String>,
    op_sys: Option<String>,
    rep_platform: Option<String>,
    alias: Option<String>,
    url: Option<String>,
    whiteboard: Option<String>,
    target_milestone: Option<String>,
    deadline: Option<String>,
    #[serde(default)]
    blocks: Vec<u64>,
    #[serde(default)]
    depends_on: Vec<u64>,
    #[serde(default)]
    cc: Vec<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    groups: Vec<String>,
    #[serde(default)]
    flags: Vec<String>,
}

impl JsonCreateBug {
    /// Validate the merged fields and build the API params. `product`,
    /// `component`, and `summary` are required; `version` defaults to
    /// `"unspecified"`; `flags` and `deadline` are parsed/validated.
    fn into_params(self) -> Result<CreateBugParams> {
        let required = |value: Option<String>, field: &str| {
            value.ok_or_else(|| {
                crate::error::BzrError::InputValidation(format!(
                    "--from-json: '{field}' is required (set it in the JSON or via --{field})"
                ))
            })
        };
        let flags = crate::commands::runtime::flags::parse_flags(&self.flags)?;
        let deadline =
            crate::validation::parse_optional_date_only(self.deadline.as_deref(), "deadline")?;
        Ok(CreateBugParams {
            product: required(self.product, "product")?,
            component: required(self.component, "component")?,
            summary: required(self.summary, "summary")?,
            version: self.version.unwrap_or_else(|| "unspecified".to_string()),
            description: self.description,
            priority: self.priority,
            severity: self.severity,
            assigned_to: self.assignee,
            op_sys: self.op_sys,
            rep_platform: self.rep_platform,
            alias: self.alias,
            url: self.url,
            whiteboard: self.whiteboard,
            target_milestone: self.target_milestone,
            deadline,
            blocks: self.blocks,
            depends_on: self.depends_on,
            cc: self.cc,
            keywords: self.keywords,
            groups: self.groups,
            flags,
        })
    }
}

/// Read the `--from-json` argument: `-` is stdin, anything else a file path.
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

/// Structured `--from-json` input, preserving the top-level shape so the
/// output shape follows the *input* (a 1-element array is still a batch), not
/// the element count.
#[derive(Debug)]
enum JsonInput {
    /// A top-level object: one bug, single-result output. Boxed because a
    /// `JsonCreateBug` is far larger than the `Many` vec handle.
    One(Box<JsonCreateBug>),
    /// A top-level array: one bug per element, partial-failure output.
    Many(Vec<JsonCreateBug>),
}

/// Parse the raw `--from-json` text. A top-level object is one bug; a top-level
/// array is one bug per element (even when it holds one). Any other shape, or
/// malformed JSON, is a clean input error naming the offending position.
fn parse_json_bugs(raw: &str) -> Result<JsonInput> {
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
            Ok(JsonInput::Many(entries))
        }
        serde_json::Value::Object(_) => {
            let one = serde_json::from_value(value).map_err(|e| {
                crate::error::BzrError::InputValidation(format!("--from-json: {e}"))
            })?;
            Ok(JsonInput::One(Box::new(one)))
        }
        _ => Err(crate::error::BzrError::InputValidation(
            "--from-json expects a JSON object or an array of objects".into(),
        )),
    }
}

/// Resolve an explicit `--description`/`--description-file` for the JSON path.
/// Unlike the interactive create flow, this does NOT auto-read stdin — only an
/// explicitly-supplied value overrides the JSON `description`.
fn explicit_description(
    description: Option<&str>,
    description_file: Option<&std::path::Path>,
) -> Result<Option<String>> {
    crate::commands::runtime::shared::materialize_body_source(
        crate::commands::runtime::shared::classify_body_source(
            description,
            description_file,
            "--description",
            "--description-file",
        )?,
        "--description-file",
    )
}

/// Overlay explicit CLI flags onto a JSON entry: a CLI value (a `Some` scalar
/// or a non-empty repeatable) wins over the JSON field, applied uniformly to
/// every element of an array.
fn overlay_cli(mut json: JsonCreateBug, args: &CreateArgs) -> Result<JsonCreateBug> {
    let CreateArgs {
        product,
        component,
        summary,
        version,
        description,
        description_file,
        priority,
        severity,
        assignee,
        op_sys,
        rep_platform,
        blocks,
        depends_on,
        create_fields,
        ..
    } = args;
    // `merge_set`/`merge_vec` overwrite the target when the CLI flag was
    // supplied (a `Some` scalar / non-empty repeatable), else leave the JSON
    // value — exactly the "CLI wins" precedence.
    merge_set(&mut json.product, product.as_deref());
    merge_set(&mut json.component, component.as_deref());
    merge_set(&mut json.summary, summary.as_deref());
    merge_set(&mut json.version, version.as_deref());
    merge_set(&mut json.priority, priority.as_deref());
    merge_set(&mut json.severity, severity.as_deref());
    merge_set(&mut json.assignee, assignee.as_deref());
    merge_set(&mut json.op_sys, op_sys.as_deref());
    merge_set(&mut json.rep_platform, rep_platform.as_deref());
    merge_set(&mut json.alias, create_fields.alias.as_deref());
    merge_set(&mut json.url, create_fields.url.as_deref());
    merge_set(&mut json.whiteboard, create_fields.whiteboard.as_deref());
    merge_set(
        &mut json.target_milestone,
        create_fields.target_milestone.as_deref(),
    );
    merge_set(&mut json.deadline, create_fields.deadline.as_deref());
    if let Some(desc) = explicit_description(description.as_deref(), description_file.as_deref())? {
        json.description = Some(desc);
    }
    merge_vec(&mut json.cc, &create_fields.cc);
    merge_vec(&mut json.keywords, &create_fields.keywords);
    merge_vec(&mut json.groups, &create_fields.groups);
    merge_vec(&mut json.flags, &create_fields.flag);
    // `blocks`/`depends_on` are `Vec<u64>`; `merge_vec` is `Vec<String>`-typed,
    // so keep the equivalent guard inline.
    if !blocks.is_empty() {
        json.blocks.clone_from(blocks);
    }
    if !depends_on.is_empty() {
        json.depends_on.clone_from(depends_on);
    }
    Ok(json)
}

/// Emit a batch-create result: a partial-failure object under `--json`, or a
/// created-IDs line plus per-item failures on stderr in table mode.
fn write_batch_create(result: &BatchCreateResult, format: OutputFormat, w: &mut Writers<'_>) {
    match format {
        OutputFormat::Json | OutputFormat::Ndjson => write_result(result, "", format, w.out),
        OutputFormat::Table => {
            if !result.created.is_empty() {
                let ids: Vec<String> = result.created.iter().map(|id| format!("#{id}")).collect();
                let _ = writeln!(w.out, "Created bugs: {}", ids.join(", "));
            }
            for f in &result.failed {
                let _ = writeln!(
                    w.err,
                    "Failed to create bug (item {}): {}",
                    f.index, f.error
                );
            }
        }
    }
}

/// Build one bug from a structured JSON object or array, the `--from-json`
/// path. All entries are validated before any write, so malformed input never
/// half-creates a batch; per-element server failures use the partial-failure
/// model (exit 11).
async fn handle_from_json(
    client: &BugzillaClient,
    args: &CreateArgs,
    arg: &str,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let raw = read_from_json(arg)?;
    match parse_json_bugs(&raw)? {
        JsonInput::One(entry) => {
            let params = overlay_cli(*entry, args)?.into_params()?;
            create_and_report(client, &params, format, w).await
        }
        JsonInput::Many(entries) => {
            if entries.is_empty() {
                return Err(crate::error::BzrError::InputValidation(
                    "--from-json: empty array, nothing to create".into(),
                ));
            }
            let mut params_list = Vec::with_capacity(entries.len());
            for entry in entries {
                params_list.push(overlay_cli(entry, args)?.into_params()?);
            }
            create_batch_from_json(client, &params_list, format, w).await
        }
    }
}

/// Create one bug and report it (or preview under `--dry-run`). Shared by the
/// flag/editor path and the `--from-json` single-object path.
async fn create_and_report(
    client: &BugzillaClient,
    params: &CreateBugParams,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    if crate::commands::runtime::dry_run::enabled() {
        write_create_dry_run(params, format, w);
        return Ok(());
    }
    let id = client.create_bug(params).await?;
    write_result(
        &ActionResult::created(id, ResourceKind::Bug),
        &format!("Created bug #{id}"),
        format,
        w.out,
    );
    Ok(())
}

/// Create a batch of bugs (top-level JSON array). The array shape always yields
/// the partial-failure result — even for a single element — so an agent's
/// output handling does not depend on the element count. Exits 11 if any
/// element fails.
async fn create_batch_from_json(
    client: &BugzillaClient,
    params_list: &[CreateBugParams],
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    if crate::commands::runtime::dry_run::enabled() {
        // One coherent object for the whole batch (N pretty-printed objects
        // would not be valid JSON); `changes` carries the array of params.
        write_result(
            &DryRunResult::new(ResourceKind::Bug, &[], &params_list),
            &format!(
                "Dry run: would create {} bug(s) (no bugs created)",
                params_list.len()
            ),
            format,
            w.out,
        );
        return Ok(());
    }
    let mut created = Vec::new();
    let mut failed = Vec::new();
    for (index, params) in params_list.iter().enumerate() {
        match client.create_bug(params).await {
            Ok(id) => created.push(id),
            Err(e) => failed.push(CreateFailure {
                index,
                error: e.to_string(),
            }),
        }
    }
    let succeeded = created.len();
    let failures = failed.len();
    write_batch_create(&BatchCreateResult::new(created, failed), format, w);
    super::update::ensure_batch_complete(succeeded, failures)
}

pub(super) async fn handle(
    client: &BugzillaClient,
    args: &CreateArgs,
    format: OutputFormat,
    w: &mut Writers<'_>,
) -> Result<()> {
    let CreateArgs {
        from_json,
        template: template_name,
        summary,
        description,
        description_file,
        blocks,
        depends_on,
        create_fields,
        ..
    } = args;

    if let Some(arg) = from_json {
        return handle_from_json(client, args, arg, format, w).await;
    }

    let flags = crate::commands::runtime::flags::parse_flags(&create_fields.flag)?;
    let deadline = crate::validation::parse_optional_date_only(
        create_fields.deadline.as_deref(),
        "--deadline",
    )?;

    let resolved_description =
        resolve_description(description.as_deref(), description_file.as_deref())?;
    let editor_flow_active = resolved_description.is_none();

    let tmpl = load_template(template_name.as_deref())?;
    let merged = merge_fields(args, tmpl.as_ref())?;

    let (resolved_summary, final_description): (Option<String>, Option<String>) =
        if editor_flow_active {
            let (parsed_summary, parsed_description) =
                run_editor_flow(summary.as_deref(), &merged)?;
            (Some(parsed_summary), Some(parsed_description))
        } else {
            (summary.clone(), resolved_description)
        };

    let params = CreateBugParams {
        product: merged.product,
        component: merged.component,
        summary: resolved_summary.ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "--summary is required (or run interactively without --description, --description-file, or piped stdin to compose in $EDITOR)"
                    .into(),
            )
        })?,
        version: merged.version.unwrap_or_else(|| "unspecified".to_string()),
        description: final_description,
        priority: merged.priority,
        severity: merged.severity,
        assigned_to: merged.assigned_to,
        op_sys: merged.op_sys,
        rep_platform: merged.rep_platform,
        alias: create_fields.alias.clone(),
        url: create_fields.url.clone(),
        whiteboard: create_fields.whiteboard.clone(),
        target_milestone: create_fields.target_milestone.clone(),
        deadline,
        blocks: blocks.clone(),
        depends_on: depends_on.clone(),
        cc: create_fields.cc.clone(),
        keywords: create_fields.keywords.clone(),
        groups: create_fields.groups.clone(),
        flags,
    };
    create_and_report(client, &params, format, w).await
}

/// Emit the would-be create payload without writing, marked `"action":"dry-run"`.
/// No bug exists yet, so `ids` is empty; `changes` carries the resolved
/// `CreateBugParams`.
fn write_create_dry_run(params: &CreateBugParams, format: OutputFormat, w: &mut Writers<'_>) {
    write_result(
        &DryRunResult::new(ResourceKind::Bug, &[], params),
        &format!(
            "Dry run: would create a bug in {}/{} (no bug created)",
            params.product, params.component
        ),
        format,
        w.out,
    );
}

#[cfg(test)]
#[path = "create_tests.rs"]
mod tests;
