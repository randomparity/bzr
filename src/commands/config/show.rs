use crate::config::Config;
use crate::error::Result;
use crate::output::resources::config::{write_config, ConfigView};
use crate::output::writers::Writers;
use crate::types::OutputFormat;

pub(super) fn handle(format: OutputFormat, w: &mut Writers<'_>) -> Result<()> {
    let config = Config::load()?;
    let path = Config::path()?;
    let view = ConfigView::from_config(&config, &path);
    write_config(&view, format, w.out);
    Ok(())
}
