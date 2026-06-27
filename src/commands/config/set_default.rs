use crate::commands::runtime::invocation::CommandContext;
use crate::config::Config;
use crate::error::Result;
use crate::output::result_types::{write_result, ConfigResult};
use crate::output::writers::Writers;

pub(super) fn handle(name: &str, ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    Config::update_locked_at(ctx.config_path_override(), |config| {
        if !config.servers.contains_key(name) {
            return Err(crate::error::BzrError::config(format!(
                "server '{name}' not found"
            )));
        }
        config.default_server = Some(name.to_string());
        Ok(())
    })?;
    let path = Config::path_at(ctx.config_path_override())?;

    write_result(
        &ConfigResult::default_set(name, path.to_string_lossy()),
        &format!(
            "Default server set to '{name}'\nConfig file: {}",
            path.display()
        ),
        ctx.format(),
        w.out,
    );
    Ok(())
}

#[cfg(test)]
#[path = "set_default_tests.rs"]
mod tests;
