//! Template management commands.
//!
//! Template operations are pure local file I/O — no network client needed.

use crate::cli::TemplateAction;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::{self, Writers};
use crate::types::BugTemplate;
use crate::types::OutputFormat;

#[expect(
    clippy::unused_async,
    reason = "async for signature consistency with sibling execute fns"
)]
pub async fn execute(
    action: &TemplateAction,
    _server: Option<&str>,
    format: OutputFormat,
    _api: Option<crate::types::ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        TemplateAction::Save {
            name,
            product,
            component,
            version,
            priority,
            severity,
            assignee,
            op_sys,
            rep_platform,
            description,
        } => {
            let template = BugTemplate {
                product: product.clone(),
                component: component.clone(),
                version: version.clone(),
                priority: priority.clone(),
                severity: severity.clone(),
                assignee: assignee.clone(),
                op_sys: op_sys.clone(),
                rep_platform: rep_platform.clone(),
                description: description.clone(),
            };

            // Require at least one field to be set
            if template.product.is_none()
                && template.component.is_none()
                && template.version.is_none()
                && template.priority.is_none()
                && template.severity.is_none()
                && template.assignee.is_none()
                && template.op_sys.is_none()
                && template.rep_platform.is_none()
                && template.description.is_none()
            {
                return Err(BzrError::InputValidation(
                    "template must have at least one field set".into(),
                ));
            }

            let mut config = Config::load()?;
            let is_update = config.templates.contains_key(name.as_str());
            config.templates.insert(name.clone(), template);
            config.save()?;

            let verb = if is_update { "Updated" } else { "Saved" };
            output::write_template_saved(name, verb, format, w.out);
        }
        TemplateAction::List => {
            let config = Config::load()?;
            output::write_template_list(&config.templates, format, w.out);
        }
        TemplateAction::Show { name } => {
            let config = Config::load()?;
            let template = config
                .templates
                .get(name.as_str())
                .ok_or_else(|| BzrError::config(format!("template '{name}' not found")))?;
            output::write_template_detail(name, template, format, w.out);
        }
        TemplateAction::Delete { name } => {
            let mut config = Config::load()?;
            if config.templates.remove(name.as_str()).is_none() {
                return Err(BzrError::config(format!("template '{name}' not found")));
            }
            config.save()?;

            output::write_template_saved(name, "Deleted", format, w.out);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "template_tests.rs"]
mod tests;
