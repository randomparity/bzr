use crate::commands::runtime::context::CommandContext;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::resources::query::write_query_detail;
use crate::output::writers::Writers;

pub(super) fn handle(
    args: &crate::cli::ShowArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let name = &args.name;
    let config = Config::load_at(ctx.config_path_override())?;
    let query = config
        .queries
        .get(name.as_str())
        .ok_or_else(|| BzrError::config(format!("query '{name}' not found")))?;
    write_query_detail(name, query, ctx.format(), w.out);
    Ok(())
}

#[cfg(test)]
#[path = "show_tests.rs"]
mod tests;
