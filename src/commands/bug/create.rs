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

/// Resolve the description source by precedence (#1-#3); the editor
/// flow (#4) is wired in a follow-up task. Returns `None` when no
/// explicit source is supplied and stdin is a TTY -- the caller must
/// then either dispatch the editor flow or reject the invocation.
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
                "no description supplied (stdin is empty and editor flow inactive)".into(),
            ));
        }
        return Ok(Some(buf));
    }
    Ok(None)
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

    // Until the editor flow lands, treat editor-flow-active as
    // an unsupported state: the resolution chain falls through with
    // no description source.
    if editor_flow_active {
        return Err(crate::error::BzrError::InputValidation(
            "a description is required: pass --description, --description-file, or pipe via stdin (interactive editor not yet supported)".into(),
        ));
    }

    // Load template defaults if specified
    let tmpl = if let Some(name) = template_name {
        let config = crate::config::Config::load()?;
        let t = config.templates.get(name.as_str()).ok_or_else(|| {
            crate::error::BzrError::config(format!("template '{name}' not found"))
        })?;
        Some(t.clone())
    } else {
        None
    };

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

    let params = CreateBugParams {
        product: resolved_product,
        component: resolved_component,
        summary: summary.clone().ok_or_else(|| {
            crate::error::BzrError::InputValidation(
                "--summary is required unless the editor flow is active".into(),
            )
        })?,
        version: version
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.version.clone()))
            .unwrap_or_else(|| "unspecified".to_string()),
        description: resolved_description
            .or_else(|| tmpl.as_ref().and_then(|t| t.description.clone())),
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
