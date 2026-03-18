use crate::config::ApiMode;
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn execute(
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::build_client(server, api).await?;
    let whoami = client.whoami().await?;
    output::print_whoami(&whoami, format);
    Ok(())
}
