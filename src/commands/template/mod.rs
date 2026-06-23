//! Template management commands.
//!
//! Template operations are pure local file I/O — no network client needed.

use crate::cli::TemplateAction;
use crate::commands::runtime::context::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::writers::Writers;
use crate::types::template::BugTemplate;

mod delete;
mod list;
mod save;
mod show;
mod update;

#[expect(
    clippy::unused_async,
    reason = "async for signature consistency with sibling execute fns"
)]
pub(crate) async fn execute(
    action: &TemplateAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        TemplateAction::Save { name, fields } => save::handle(name, fields, ctx, w),
        TemplateAction::List => list::handle(ctx, w),
        TemplateAction::Show { name } => show::handle(name, ctx, w),
        TemplateAction::Update(args) => update::handle(args, ctx, w),
        TemplateAction::Delete { name } => delete::handle(name, ctx, w),
    }
}

/// Whether a template has no fields set (used to reject empty saves/updates).
fn template_is_empty(template: &BugTemplate) -> bool {
    template.product.is_none()
        && template.component.is_none()
        && template.version.is_none()
        && template.priority.is_none()
        && template.severity.is_none()
        && template.assignee.is_none()
        && template.op_sys.is_none()
        && template.rep_platform.is_none()
        && template.description.is_none()
        && template.url.is_none()
        && template.whiteboard.is_none()
        && template.target_milestone.is_none()
        && template.deadline.is_none()
        && template.cc.is_empty()
        && template.keywords.is_empty()
        && template.groups.is_empty()
        && template.flags.is_empty()
}

/// Validate template defaults that share parsing rules with `bug create`.
fn validate_template(template: &mut BugTemplate) -> Result<()> {
    template.deadline =
        crate::validation::parse_optional_date_only(template.deadline.as_deref(), "--deadline")?;
    crate::commands::runtime::flags::parse_flags(&template.flags)?;
    Ok(())
}

/// Reset the named field to unset. Most names match the long flag (kebab-case).
fn clear_template_field(template: &mut BugTemplate, field: &str) -> Result<()> {
    match field {
        "product" => template.product = None,
        "component" => template.component = None,
        "version" => template.version = None,
        "priority" => template.priority = None,
        "severity" => template.severity = None,
        "assignee" => template.assignee = None,
        "op-sys" => template.op_sys = None,
        "rep-platform" => template.rep_platform = None,
        "description" => template.description = None,
        "url" => template.url = None,
        "whiteboard" => template.whiteboard = None,
        "target-milestone" => template.target_milestone = None,
        "deadline" => template.deadline = None,
        "cc" => template.cc.clear(),
        "keywords" => template.keywords.clear(),
        "groups" => template.groups.clear(),
        "flag" | "flags" => template.flags.clear(),
        other => {
            return Err(BzrError::InputValidation(format!(
                "unknown --clear field '{other}'; valid fields: product, component, \
                 version, priority, severity, assignee, op-sys, rep-platform, description, url, \
                 whiteboard, target-milestone, deadline, cc, keywords, groups, flag, flags"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
