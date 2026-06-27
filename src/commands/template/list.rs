use crate::commands::runtime::invocation::CommandContext;
use crate::config::Config;
use crate::error::Result;
use crate::output::resources::template::write_template_list;
use crate::output::writers::Writers;

pub(super) fn handle(ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let config = Config::load_at(ctx.config_path_override())?;
    write_template_list(&config.templates, ctx.format(), w.out);
    Ok(())
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
