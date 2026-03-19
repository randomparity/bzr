use crate::cli::ServerAction;
use crate::config::ApiMode;
use crate::error::Result;
use crate::output;
use crate::types::OutputFormat;

pub async fn execute(
    action: &ServerAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::build_client(server, api).await?;

    match action {
        ServerAction::Info => {
            let version = client.server_version().await?;
            let extensions = client.server_extensions().await?;
            output::print_server_info(&version, &extensions, format);
        }
    }
    Ok(())
}
