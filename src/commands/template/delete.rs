use crate::commands::runtime::context::CommandContext;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::resources::template::write_template_saved;
use crate::output::writers::Writers;

pub(super) fn handle(name: &str, ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    Config::update_locked_at(ctx.config_path_override(), |config| {
        if config.templates.remove(name).is_none() {
            return Err(BzrError::config(format!("template '{name}' not found")));
        }
        Ok(())
    })?;

    write_template_saved(name, "Deleted", ctx.format(), w.out);
    Ok(())
}
