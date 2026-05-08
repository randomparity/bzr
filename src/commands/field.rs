use crate::cli::FieldAction;
use crate::error::Result;
use crate::output::{self, Writers};
use crate::types::ApiMode;
use crate::types::OutputFormat;

pub async fn execute(
    action: &FieldAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    match action {
        FieldAction::Aliases => {
            output::write_field_aliases(crate::client::FIELD_ALIASES, format, w.out);
            return Ok(());
        }
        FieldAction::List { name } => {
            let client = super::shared::connect_and_configure(server, api).await?;
            let values = client.get_field_values(name).await?;
            if values.is_empty() && format == OutputFormat::Table {
                let _ = writeln!(w.out, "No values for field '{name}'.");
            } else {
                output::write_field_values(&values, format, w.out);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
