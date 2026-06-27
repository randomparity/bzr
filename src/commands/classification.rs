use crate::cli::ClassificationAction;
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::classification::{write_classification, write_classifications};
use crate::output::writers::Writers;

pub(crate) async fn execute(
    action: &ClassificationAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();

    match action {
        ClassificationAction::List { projection } => {
            let projection = crate::validation::fields::projection_for(
                format,
                projection.fields.as_deref(),
                projection.exclude_fields.as_deref(),
                crate::types::classification::CLASSIFICATION_FIELDS,
                w.err,
            )?;
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            let classifications = client.list_classifications().await?;
            // A lone "Unclassified" is Bugzilla's signal that classifications
            // are disabled on this server.
            let disabled = matches!(
                classifications.as_slice(),
                [only]
                    if only
                        .name
                        .as_deref()
                        .is_some_and(|name| name.eq_ignore_ascii_case("Unclassified"))
            );
            if disabled {
                let _ = writeln!(
                    w.err,
                    "Note: only the default 'Unclassified' classification exists; \
                     this server likely has classifications disabled."
                );
            }
            write_classifications(&classifications, format, &projection, w.out);
        }
        ClassificationAction::View { name, projection } => {
            let projection = crate::validation::fields::projection_for(
                format,
                projection.fields.as_deref(),
                projection.exclude_fields.as_deref(),
                crate::types::classification::CLASSIFICATION_FIELDS,
                w.err,
            )?;
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            let classification = client.get_classification(name).await?;
            write_classification(&classification, format, &projection, w.out);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "classification_tests.rs"]
mod tests;
