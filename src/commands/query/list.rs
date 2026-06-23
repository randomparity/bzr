use crate::commands::runtime::context::CommandContext;
use crate::config::Config;
use crate::error::Result;
use crate::output::resources::query::write_query_list;
use crate::output::writers::Writers;

pub(super) fn handle(ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let config = Config::load_at(ctx.config_path_override())?;
    write_query_list(&config.queries, ctx.format(), w.out);
    Ok(())
}
