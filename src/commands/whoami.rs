//! Whoami command — shows the authenticated user's identity.

use crate::cli::WhoamiAction;
use crate::error::Result;
use crate::output::resources::user::write_whoami;
use crate::output::writers::Writers;
use crate::types::ApiMode;
use crate::types::OutputFormat;

pub async fn execute(
    _action: &WhoamiAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    let client = super::shared::connect_and_configure(server, api).await?;
    let whoami = client.whoami().await?;
    write_whoami(&whoami, format, w.out);
    Ok(())
}

#[cfg(test)]
#[path = "whoami_tests.rs"]
mod tests;
