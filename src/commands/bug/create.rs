use std::io::IsTerminal;

use crate::cli::CreateArgs;
use crate::client::BugzillaClient;
use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::editor;
use crate::error::Result;
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
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
    url: Option<String>,
    whiteboard: Option<String>,
    target_milestone: Option<String>,
    deadline: Option<String>,
    cc: Vec<String>,
    keywords: Vec<String>,
    groups: Vec<String>,
    flags: Vec<String>,
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

fn load_template(
    name: Option<&str>,
    config_path_override: Option<&std::path::Path>,
) -> Result<Option<crate::types::BugTemplate>> {
    let Some(name) = name else { return Ok(None) };
    let config = crate::config::Config::load_at(config_path_override)?;
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
        create_fields,
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
        url: create_fields
            .url
            .clone()
            .or_else(|| tmpl.and_then(|t| t.url.clone())),
        whiteboard: create_fields
            .whiteboard
            .clone()
            .or_else(|| tmpl.and_then(|t| t.whiteboard.clone())),
        target_milestone: create_fields
            .target_milestone
            .clone()
            .or_else(|| tmpl.and_then(|t| t.target_milestone.clone())),
        deadline: create_fields
            .deadline
            .clone()
            .or_else(|| tmpl.and_then(|t| t.deadline.clone())),
        cc: merge_template_vec(&create_fields.cc, tmpl, |t| &t.cc),
        keywords: merge_template_vec(&create_fields.keywords, tmpl, |t| &t.keywords),
        groups: merge_template_vec(&create_fields.groups, tmpl, |t| &t.groups),
        flags: merge_template_vec(&create_fields.flag, tmpl, |t| &t.flags),
        template_description: tmpl.and_then(|t| t.description.clone()),
    })
}

fn merge_template_vec(
    cli_values: &[String],
    tmpl: Option<&crate::types::BugTemplate>,
    template_values: impl FnOnce(&crate::types::BugTemplate) -> &[String],
) -> Vec<String> {
    if cli_values.is_empty() {
        tmpl.map(template_values).unwrap_or_default().to_vec()
    } else {
        cli_values.to_vec()
    }
}

/// Create one bug and report it (or preview under `--dry-run`). Shared by the
/// flag/editor path and the `--from-json` single-object path.
pub(super) async fn create_and_report(
    client: &BugzillaClient,
    params: &CreateBugParams,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    if ctx.dry_run() {
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

pub(super) async fn handle(
    client: &BugzillaClient,
    args: &CreateArgs,
    ctx: &CommandContext,
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
        return super::create_json::handle(client, args, arg, ctx, w).await;
    }

    let resolved_description =
        resolve_description(description.as_deref(), description_file.as_deref())?;
    let editor_flow_active = resolved_description.is_none();

    let tmpl = load_template(template_name.as_deref(), ctx.config_path_override())?;
    let merged = merge_fields(args, tmpl.as_ref())?;
    let flags = crate::commands::runtime::flags::parse_flags(&merged.flags)?;
    let deadline =
        crate::validation::parse_optional_date_only(merged.deadline.as_deref(), "--deadline")?;

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
        url: merged.url,
        whiteboard: merged.whiteboard,
        target_milestone: merged.target_milestone,
        deadline,
        blocks: blocks.clone(),
        depends_on: depends_on.clone(),
        cc: merged.cc,
        keywords: merged.keywords,
        groups: merged.groups,
        flags,
    };
    create_and_report(client, &params, ctx, w).await
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
