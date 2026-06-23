use crate::config::Config;
use crate::error::Result;
use crate::output::result_types::{write_result, ConfigResult};
use crate::output::writers::Writers;
use crate::types::OutputFormat;

pub(super) fn handle(name: &str, format: OutputFormat, w: &mut Writers<'_>) -> Result<()> {
    Config::update_locked(|config| {
        if !config.servers.contains_key(name) {
            return Err(crate::error::BzrError::config(format!(
                "server '{name}' not found"
            )));
        }
        config.default_server = Some(name.to_string());
        Ok(())
    })?;
    let path = Config::path()?;

    write_result(
        &ConfigResult::default_set(name, path.to_string_lossy()),
        &format!(
            "Default server set to '{name}'\nConfig file: {}",
            path.display()
        ),
        format,
        w.out,
    );
    Ok(())
}
