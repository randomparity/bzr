use crate::cli::ClassificationAction;
use crate::error::Result;
use crate::output::{self, OutputFormat};

pub async fn execute(
    action: &ClassificationAction,
    server: Option<&str>,
    format: OutputFormat,
) -> Result<()> {
    let client = super::shared::build_client(server).await?;

    match action {
        ClassificationAction::View { name } => {
            let classification = client.get_classification(name).await?;
            output::print_classification(&classification, format);
        }
    }
    Ok(())
}
