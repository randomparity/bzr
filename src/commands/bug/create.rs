use crate::cli::BugAction;
use crate::client::BugzillaClient;
use crate::error::Result;
use crate::output::{self, ActionResult, ResourceKind};
use crate::types::{CreateBugParams, OutputFormat};

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

    // description_file resolution lands in the next task
    let _ = description_file;

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
        summary: summary.clone(),
        version: version
            .clone()
            .or_else(|| tmpl.as_ref().and_then(|t| t.version.clone()))
            .unwrap_or_else(|| "unspecified".to_string()),
        description: description
            .clone()
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
