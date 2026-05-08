use crate::cli::ClassificationAction;
use crate::error::Result;
use crate::output::{self, Writers};
use crate::types::ApiMode;
use crate::types::OutputFormat;

pub async fn execute(
    action: &ClassificationAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    let client = super::shared::connect_and_configure(server, api).await?;

    match action {
        ClassificationAction::View { name } => {
            let classification = client.get_classification(name).await?;
            output::write_classification(&classification, format, w.out);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "classification_tests.rs"]
mod tests;
