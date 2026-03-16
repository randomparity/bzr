use crate::cli::FieldAction;
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn execute(
    action: &FieldAction,
    server: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let client = super::shared::build_client(server).await?;

    match action {
        FieldAction::List { name } => {
            let values = client.get_field_values(name).await?;
            output::print_field_values(name, &values, format);
        }
    }
    Ok(())
}
