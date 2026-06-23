//! Template management commands.
//!
//! Template operations are pure local file I/O — no network client needed.

use crate::cli::TemplateAction;
use crate::commands::runtime::context::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::writers::Writers;
use crate::types::BugTemplate;

mod delete;
mod list;
mod save;
mod show;
mod update;

#[expect(
    clippy::unused_async,
    reason = "async for signature consistency with sibling execute fns"
)]
pub async fn execute(
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
fn template_is_empty(t: &BugTemplate) -> bool {
    t.product.is_none()
        && t.component.is_none()
        && t.version.is_none()
        && t.priority.is_none()
        && t.severity.is_none()
        && t.assignee.is_none()
        && t.op_sys.is_none()
        && t.rep_platform.is_none()
        && t.description.is_none()
        && t.url.is_none()
        && t.whiteboard.is_none()
        && t.target_milestone.is_none()
        && t.deadline.is_none()
        && t.cc.is_empty()
        && t.keywords.is_empty()
        && t.groups.is_empty()
        && t.flags.is_empty()
}

/// Validate template defaults that share parsing rules with `bug create`.
fn validate_template(t: &mut BugTemplate) -> Result<()> {
    t.deadline = crate::validation::parse_optional_date_only(t.deadline.as_deref(), "--deadline")?;
    crate::commands::runtime::flags::parse_flags(&t.flags)?;
    Ok(())
}

/// Reset the named field to unset. Most names match the long flag (kebab-case).
fn clear_template_field(t: &mut BugTemplate, field: &str) -> Result<()> {
    match field {
        "product" => t.product = None,
        "component" => t.component = None,
        "version" => t.version = None,
        "priority" => t.priority = None,
        "severity" => t.severity = None,
        "assignee" => t.assignee = None,
        "op-sys" => t.op_sys = None,
        "rep-platform" => t.rep_platform = None,
        "description" => t.description = None,
        "url" => t.url = None,
        "whiteboard" => t.whiteboard = None,
        "target-milestone" => t.target_milestone = None,
        "deadline" => t.deadline = None,
        "cc" => t.cc.clear(),
        "keywords" => t.keywords.clear(),
        "groups" => t.groups.clear(),
        "flag" | "flags" => t.flags.clear(),
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
