use crate::commands::runtime::context::CommandContext;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::resources::template::write_template_detail;
use crate::output::writers::Writers;

pub(super) fn handle(name: &str, ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let config = Config::load_at(ctx.config_path_override())?;
    let template = config
        .templates
        .get(name)
        .ok_or_else(|| BzrError::config(format!("template '{name}' not found")))?;
    write_template_detail(name, template, ctx.format(), w.out);
    Ok(())
}

#[cfg(test)]
#[path = "show_tests.rs"]
mod tests;
