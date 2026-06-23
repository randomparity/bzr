use crate::cli::TemplateUpdateArgs;
use crate::commands::runtime::context::CommandContext;
use crate::commands::runtime::shared::{merge_set, merge_vec};
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

    if super::template_is_empty(&fields.to_template()) && clear.is_empty() {
        return Err(BzrError::InputValidation(
            "no changes specified: provide a field flag or --clear <field>".into(),
        ));
    }

    Config::update_locked_at(ctx.config_path_override(), |config| {
        let Some(template) = config.templates.get_mut(name.as_str()) else {
            return Err(BzrError::config(format!("template '{name}' not found")));
        };
        merge_set(&mut template.product, fields.product.as_deref());
        merge_set(&mut template.component, fields.component.as_deref());
        merge_set(&mut template.version, fields.version.as_deref());
        merge_set(&mut template.priority, fields.priority.as_deref());
        merge_set(&mut template.severity, fields.severity.as_deref());
        merge_set(&mut template.assignee, fields.assignee.as_deref());
        merge_set(&mut template.op_sys, fields.op_sys.as_deref());
        merge_set(&mut template.rep_platform, fields.rep_platform.as_deref());
        merge_set(&mut template.description, fields.description.as_deref());
        merge_set(&mut template.url, fields.url.as_deref());
        merge_set(&mut template.whiteboard, fields.whiteboard.as_deref());
        merge_set(
            &mut template.target_milestone,
            fields.target_milestone.as_deref(),
        );
        merge_set(&mut template.deadline, fields.deadline.as_deref());
        merge_vec(&mut template.cc, &fields.cc);
        merge_vec(&mut template.keywords, &fields.keywords);
        merge_vec(&mut template.groups, &fields.groups);
        merge_vec(&mut template.flags, &fields.flag);
        for field in clear {
            super::clear_template_field(template, field)?;
        }
        super::validate_template(template)?;
        if super::template_is_empty(template) {
            return Err(BzrError::InputValidation(
                "update would clear all fields; a template must keep at least one field set".into(),
            ));
        }
        Ok(())
    })?;

    write_template_saved(name, "Updated", ctx.format(), w.out);
    Ok(())
}
