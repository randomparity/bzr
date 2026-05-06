use std::io::{IsTerminal, Read};

use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::{self, ActionResult, ResourceKind};
use crate::types::{CreateBugParams, OutputFormat};

fn read_description_file(path: &std::path::Path) -> Result<String> {
    if !path.exists() {
        return Err(crate::error::BzrError::InputValidation(format!(
            "--description-file path does not exist: {}",
            path.display()
        )));
    }
    std::fs::read_to_string(path).map_err(|e| {
        crate::error::BzrError::InputValidation(format!(
            "--description-file could not be read ({}): {e}",
            path.display()
        ))
    })
}

const SENTINEL: &str = "# ------------------------ >8 ------------------------";

#[expect(
    clippy::too_many_arguments,
    reason = "field-by-field preview struct, ≤8 args is fine here"
)]
fn build_preview_params(
    product: &str,
    component: &str,
    version: Option<String>,
    priority: Option<String>,
    severity: Option<String>,
    assigned_to: Option<String>,
    op_sys: Option<String>,
    rep_platform: Option<String>,
) -> CreateBugParams {
    CreateBugParams {
        product: product.to_string(),
        component: component.to_string(),
        summary: String::new(),
        version: version.unwrap_or_else(|| "unspecified".to_string()),
        description: None,
        priority,
        severity,
        assigned_to,
        op_sys,
        rep_platform,
        blocks: vec![],
        depends_on: vec![],
        cc: vec![],
        keywords: vec![],
    }
}

/// Parse the post-editor buffer into `(summary, description)`.
///
/// Truncates at the sentinel divider, takes the first non-empty
/// line as the summary, and joins the remaining lines (after
/// stripping leading and trailing blank lines) as the description.
/// Returns `InputValidation` when no non-empty line is found above
/// the sentinel.
fn parse_editor_buffer(raw: &str) -> Result<(String, String)> {
    let truncated: String = raw
        .lines()
        .take_while(|l| l.trim_end() != SENTINEL)
        .collect::<Vec<_>>()
        .join("\n");

    let mut lines = truncated.lines();
    let summary = loop {
        match lines.next() {
            Some(line) if line.trim().is_empty() => {}
            Some(line) => break line.trim().to_string(),
            None => {
                return Err(crate::error::BzrError::InputValidation(
                    "empty buffer, aborting".into(),
                ));
            }
        }
    };

    let mut rest: Vec<&str> = lines.collect();
    while rest.first().is_some_and(|l| l.trim().is_empty()) {
        rest.remove(0);
    }
    while rest.last().is_some_and(|l| l.trim().is_empty()) {
        rest.pop();
    }
    let description = rest.join("\n");

    Ok((summary, description))
}

/// Build the pre-filled `$EDITOR` buffer.
///
/// Layout: optional pre-filled summary on line 1, optional template
/// description body, the sentinel divider, and a read-only field
/// reminder block populated from `params`.
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

/// Resolve the description source by precedence (#1-#3): explicit
/// `--description`, `--description-file`, or piped stdin. Returns
/// `None` when no explicit source is supplied and stdin is a TTY --
/// the caller then dispatches the `$EDITOR` flow (#4).
fn resolve_description(
    description: Option<&str>,
    description_file: Option<&std::path::Path>,
) -> Result<Option<String>> {
    if let Some(d) = description {
        return Ok(Some(d.to_owned()));
    }
    if let Some(p) = description_file {
        return Ok(Some(read_description_file(p)?));
    }
    if !std::io::stdin().is_terminal() {
        let mut buf = String::new();
        std::io::stdin().lock().read_to_string(&mut buf)?;
        if buf.trim().is_empty() {
            return Err(crate::error::BzrError::InputValidation(
                "no description supplied (piped stdin is empty)".into(),
            ));
        }
        return Ok(Some(buf));
    }
    Ok(None)
}

/// Resolve the optional `--template` reference into a cloned
/// `BugTemplate`, surfacing a config error when the name is unknown.
fn load_template(name: Option<&str>) -> Result<Option<crate::types::BugTemplate>> {
    let Some(name) = name else { return Ok(None) };
    let config = crate::config::Config::load()?;
    let t = config
        .templates
        .get(name)
        .ok_or_else(|| crate::error::BzrError::config(format!("template '{name}' not found")))?;
    Ok(Some(t.clone()))
}

/// Drive the `$EDITOR` flow: build the pre-filled buffer from the
/// resolved field defaults, launch the editor, and parse the result.
fn run_editor_flow(
    summary_pre_fill: Option<&str>,
    template_description: Option<&str>,
    preview: &CreateBugParams,
) -> Result<(String, String)> {
    let template_buf = build_editor_template(summary_pre_fill, template_description, preview);
    let raw = super::super::editor::launch(&template_buf, "bug-create")?;
    parse_editor_buffer(&raw)
}

/// Build the editor preview params from the merged CLI/template
/// field set, then run the editor flow and return the user-edited
/// `(summary, description)`.
fn dispatch_editor_flow(
    action: &BugAction,
    resolved_product: &str,
    resolved_component: &str,
    tmpl: Option<&crate::types::BugTemplate>,
) -> Result<(String, String)> {
    let BugAction::Create {
        summary,
        version,
        priority,
        severity,
        assignee,
        op_sys,
        rep_platform,
        ..
    } = action
    else {
        unreachable!()
    };
    let template_description_default = tmpl.and_then(|t| t.description.clone());
    let preview = build_preview_params(
        resolved_product,
        resolved_component,
        version
            .clone()
            .or_else(|| tmpl.and_then(|t| t.version.clone())),
        priority
            .clone()
            .or_else(|| tmpl.and_then(|t| t.priority.clone())),
        severity
            .clone()
            .or_else(|| tmpl.and_then(|t| t.severity.clone())),
        assignee
            .clone()
            .or_else(|| tmpl.and_then(|t| t.assignee.clone())),
        op_sys
            .clone()
            .or_else(|| tmpl.and_then(|t| t.op_sys.clone())),
        rep_platform
            .clone()
            .or_else(|| tmpl.and_then(|t| t.rep_platform.clone())),
    );
    run_editor_flow(
        summary.as_deref(),
        template_description_default.as_deref(),
        &preview,
    )
}

pub(super) async fn handle(
    client: &BugzillaClient,
    action: &BugAction,
    format: OutputFormat,
) -> Result<()> {
    let BugAction::Create {
        template: template_name,
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
    } = action
    else {
        unreachable!()
    };

    let resolved_description =
        resolve_description(description.as_deref(), description_file.as_deref())?;

    // The editor flow (when stdin is a TTY and no explicit source)
    // produces both summary and description. Outside that flow,
    // --summary must be supplied.
    let editor_flow_active = resolved_description.is_none() && std::io::stdin().is_terminal();

    let tmpl = load_template(template_name.as_deref())?;

    // Merge: CLI flags win over template defaults
    let resolved_product = product
        .clone()
        .or_else(|| tmpl.as_ref().and_then(|t| t.product.clone()))
        .ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "--product is required (provide it directly or via a template)".into(),
            )
        })?;
    let resolved_component = component
        .clone()
        .or_else(|| tmpl.as_ref().and_then(|t| t.component.clone()))
        .ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "--component is required (provide it directly or via a template)".into(),
            )
        })?;

    let (resolved_summary, final_description): (Option<String>, Option<String>) =
        if editor_flow_active {
            let (parsed_summary, parsed_description) = dispatch_editor_flow(
                action,
                &resolved_product,
                &resolved_component,
                tmpl.as_ref(),
            )?;
            // The user's edit is authoritative: an empty description
            // means the user explicitly cleared the body, so we keep
            // Some("") rather than falling back to the template.
            (Some(parsed_summary), Some(parsed_description))
        } else {
            (summary.clone(), resolved_description)
        };

    let params = CreateBugParams {
        product: resolved_product,
        component: resolved_component,
        summary: resolved_summary.ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "--summary is required unless the editor flow is active".into(),
            )
        })?,
        version: version
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.version.clone()))
            .unwrap_or_else(|| "unspecified".to_string()),
        description: final_description,
        priority: priority
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.priority.clone())),
        severity: severity
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.severity.clone())),
        assigned_to: assignee
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.assignee.clone())),
        op_sys: op_sys
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.op_sys.clone())),
        rep_platform: rep_platform
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.rep_platform.clone())),
        blocks: blocks.clone(),
        depends_on: depends_on.clone(),
        cc: vec![],
        keywords: vec![],
    };
    let id = client.create_bug(&params).await?;
    output::print_result(
        &ActionResult::created(id, ResourceKind::Bug),
        &format!("Created bug #{id}"),
        format,
    );
    Ok(())
}

#[cfg(test)]
#[path = "create_tests.rs"]
mod tests;
