//! Template management commands.
//!
//! Template operations are pure local file I/O — no network client needed.

use crate::cli::TemplateAction;
use crate::commands::runtime::invocation::CommandContext;
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

/// Validate template defaults that share parsing rules with `bug create`.
fn validate_template(template: &mut BugTemplate) -> Result<()> {
    template.deadline =
        crate::validation::parse_optional_date_only(template.deadline.as_deref(), "--deadline")?;
    crate::commands::runtime::input::flags::parse_flags(&template.flags)?;
    Ok(())
}

/// Reset the named field to unset. Most names match the long flag (kebab-case).
fn clear_template_field(template: &mut BugTemplate, field: &str) -> Result<()> {
    if template.clear_field(field) {
        return Ok(());
    }
    Err(BzrError::InputValidation(format!(
        "unknown --clear field '{field}'; valid fields: {}",
        BugTemplate::clearable_fields().join(", ")
    )))
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
