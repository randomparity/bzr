use crate::cli::TemplateUpdateArgs;
use crate::commands::runtime::invocation::CommandContext;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::resources::template::write_template_saved;
use crate::output::writers::Writers;

/// Merge `template update` flags into an existing template in place. A field
/// flag replaces that field; `--clear <field>` resets it; omitted flags are
/// left unchanged. Rejects a no-op call and a result with no fields set.
pub(super) fn handle(
    args: &TemplateUpdateArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let TemplateUpdateArgs {
        name,
        fields,
        clear,
    } = args;

    if fields.is_empty_change() && clear.is_empty() {
        return Err(BzrError::input(
            "no changes specified: provide a field flag or --clear <field>".into(),
        ));
    }

    Config::update_locked_at(ctx.config_path_override(), |config| {
        let Some(template) = config.templates.get_mut(name.as_str()) else {
            return Err(BzrError::config(format!("template '{name}' not found")));
        };
        template.merge_from(&fields.to_template());
        for field in clear {
            super::clear_template_field(template, field)?;
        }
        super::validate_template(template)?;
        if template.is_empty() {
            return Err(BzrError::input(
                "update would clear all fields; a template must keep at least one field set".into(),
            ));
        }
        Ok(())
    })?;

    write_template_saved(name, "Updated", ctx.format(), w.out);
    Ok(())
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
