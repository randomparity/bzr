//! Template management commands.
//!
//! Template operations are pure local file I/O — no network client needed.

use crate::cli::TemplateAction;
use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::shared::{merge_set, merge_vec};
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::resources::template::{
    write_template_detail, write_template_list, write_template_saved,
};
use crate::output::writers::Writers;
use crate::types::BugTemplate;

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
        TemplateAction::Save { name, fields } => {
            let mut template = fields.to_template();
            validate_template(&mut template)?;

            // Require at least one field to be set
            if template_is_empty(&template) {
                return Err(BzrError::InputValidation(
                    "template must have at least one field set".into(),
                ));
            }

            let mut is_update = false;
            Config::update_locked_at(ctx.config_path_override(), |config| {
                is_update = config.templates.contains_key(name.as_str());
                config.templates.insert(name.clone(), template);
                Ok(())
            })?;

            let verb = if is_update { "Updated" } else { "Saved" };
            write_template_saved(name, verb, ctx.format(), w.out);
        }
        TemplateAction::List => {
            let config = Config::load_at(ctx.config_path_override())?;
            write_template_list(&config.templates, ctx.format(), w.out);
        }
        TemplateAction::Show { name } => {
            let config = Config::load_at(ctx.config_path_override())?;
            let template = config
                .templates
                .get(name.as_str())
                .ok_or_else(|| BzrError::config(format!("template '{name}' not found")))?;
            write_template_detail(name, template, ctx.format(), w.out);
        }
        TemplateAction::Update(args) => handle_update(args, ctx, w)?,
        TemplateAction::Delete { name } => {
            Config::update_locked_at(ctx.config_path_override(), |config| {
                if config.templates.remove(name.as_str()).is_none() {
                    return Err(BzrError::config(format!("template '{name}' not found")));
                }
                Ok(())
            })?;

            write_template_saved(name, "Deleted", ctx.format(), w.out);
        }
    }
    Ok(())
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
            )))
        }
    }
    Ok(())
}

/// Merge `template update` flags into an existing template in place. A field
/// flag replaces that field; `--clear <field>` resets it; omitted flags are
/// left unchanged. Rejects a no-op call and a result with no fields set.
fn handle_update(
    args: &crate::cli::TemplateUpdateArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let crate::cli::TemplateUpdateArgs {
        name,
        fields,
        clear,
    } = args;

    if template_is_empty(&fields.to_template()) && clear.is_empty() {
        return Err(BzrError::InputValidation(
            "no changes specified: provide a field flag or --clear <field>".into(),
        ));
    }

    Config::update_locked_at(ctx.config_path_override(), |config| {
        let Some(t) = config.templates.get_mut(name.as_str()) else {
            return Err(BzrError::config(format!("template '{name}' not found")));
        };
        merge_set(&mut t.product, fields.product.as_deref());
        merge_set(&mut t.component, fields.component.as_deref());
        merge_set(&mut t.version, fields.version.as_deref());
        merge_set(&mut t.priority, fields.priority.as_deref());
        merge_set(&mut t.severity, fields.severity.as_deref());
        merge_set(&mut t.assignee, fields.assignee.as_deref());
        merge_set(&mut t.op_sys, fields.op_sys.as_deref());
        merge_set(&mut t.rep_platform, fields.rep_platform.as_deref());
        merge_set(&mut t.description, fields.description.as_deref());
        merge_set(&mut t.url, fields.url.as_deref());
        merge_set(&mut t.whiteboard, fields.whiteboard.as_deref());
        merge_set(&mut t.target_milestone, fields.target_milestone.as_deref());
        merge_set(&mut t.deadline, fields.deadline.as_deref());
        merge_vec(&mut t.cc, &fields.cc);
        merge_vec(&mut t.keywords, &fields.keywords);
        merge_vec(&mut t.groups, &fields.groups);
        merge_vec(&mut t.flags, &fields.flag);
        for field in clear {
            clear_template_field(t, field)?;
        }
        validate_template(t)?;
        if template_is_empty(t) {
            return Err(BzrError::InputValidation(
                "update would clear all fields; a template must keep at least one field set".into(),
            ));
        }
        Ok(())
    })?;

    write_template_saved(name, "Updated", ctx.format(), w.out);
    Ok(())
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
