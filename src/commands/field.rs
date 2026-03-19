use crate::cli::FieldAction;
use crate::config::ApiMode;
use crate::error::Result;
use crate::output;
use crate::types::OutputFormat;

pub async fn execute(
    action: &FieldAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
) -> Result<()> {
    let client = super::shared::build_client(server, api).await?;

    match action {
        FieldAction::List { name } => {
            let values = client.get_field_values(name).await?;
            output::print_field_values(name, &values, format);
        }
    }
    Ok(())
}
