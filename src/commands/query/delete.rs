use crate::commands::runtime::context::CommandContext;
use crate::config::Config;
use crate::error::{BzrError, Result};
use crate::output::resources::query::write_query_saved;
use crate::output::writers::Writers;

pub(super) fn handle(
    args: &crate::cli::DeleteArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let name = &args.name;
    Config::update_locked_at(ctx.config_path_override(), |config| {
        if config.queries.remove(name.as_str()).is_none() {
            return Err(BzrError::config(format!("query '{name}' not found")));
        }
        Ok(())
    })?;

    write_query_saved(name, "Deleted", ctx.format(), w.out);
    Ok(())
}

#[cfg(test)]
#[path = "delete_tests.rs"]
mod tests;
