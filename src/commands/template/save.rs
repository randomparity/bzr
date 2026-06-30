use crate::cli::TemplateFields;
use crate::commands::runtime::invocation::CommandContext;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::resources::template::write_template_saved;
use crate::output::writers::Writers;

pub(super) fn handle(
    name: &str,
    fields: &TemplateFields,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let mut template = fields.to_template();
    super::validate_template(&mut template)?;

    if template.is_empty() {
        return Err(BzrError::input(
            "template must have at least one field set".into(),
        ));
    }

    let mut is_update = false;
    Config::update_locked_at(ctx.config_path_override(), |config| {
        is_update = config.templates.contains_key(name);
        config.templates.insert(name.to_string(), template);
        Ok(())
    })?;

    let verb = if is_update { "Updated" } else { "Saved" };
    write_template_saved(name, verb, ctx.format(), w.out);
    Ok(())
}

#[cfg(test)]
#[path = "save_tests.rs"]
mod tests;
