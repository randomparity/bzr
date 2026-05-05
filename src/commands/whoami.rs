//! Whoami command — shows the authenticated user's identity.

use crate::cli::WhoamiAction;
use crate::error::Result;
use crate::output;
use crate::types::ApiMode;
use crate::types::OutputFormat;

pub async fn execute(
    _action: &WhoamiAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::connect_and_configure(server, api).await?;
    let whoami = client.whoami().await?;
    output::print_whoami(&whoami, format);
    Ok(())
}

#[cfg(test)]
#[path = "whoami_tests.rs"]
mod tests;
