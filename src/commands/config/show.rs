use crate::commands::runtime::context::CommandContext;
use crate::config::Config;
use crate::error::Result;
use crate::output::resources::config::{write_config, ConfigView};
use crate::output::writers::Writers;

pub(super) fn handle(ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let config = Config::load_at(ctx.config_path_override())?;
    let path = Config::path_at(ctx.config_path_override())?;
    let view = ConfigView::from_config(&config, &path);
    write_config(&view, ctx.format(), w.out);
    Ok(())
}
