use crate::cli::ServerAction;
use crate::error::Result;
use crate::output::{self, Writers};
use crate::types::ApiMode;
use crate::types::OutputFormat;

pub async fn execute(
    action: &ServerAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    let client = super::shared::connect_and_configure(server, api).await?;

    match action {
        ServerAction::Info => {
            let info = client.server_info().await?;
            output::write_server_info(&info, format, w.out);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
