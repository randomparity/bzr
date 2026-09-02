use crate::cli::ClassificationAction;
use crate::commands::runtime::invocation::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::resources::classification::{write_classification, write_classifications};
use crate::output::writers::Writers;

const DISABLED_NOTE: &str =
    "Note: only the default 'Unclassified' classification exists; this server likely has classifications disabled.";

fn write_disabled_classifications(
    format: crate::types::OutputFormat,
    projection: &crate::validation::fields::FieldProjection,
    w: &mut Writers<'_>,
) {
    if format.is_json_family() {
        write_classifications(&[], format, projection, w.out);
        let _ = writeln!(w.err, "{DISABLED_NOTE}");
    } else {
        let _ = writeln!(w.out, "{DISABLED_NOTE}");
    }
}

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
            let classifications = match client.list_classifications().await {
                Ok(classifications) => classifications,
                Err(BzrError::Api { code: 900, .. }) => {
                    write_disabled_classifications(format, &projection, w);
                    return Ok(());
                }
                Err(error) => return Err(error),
            };
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
                write_disabled_classifications(format, &projection, w);
                return Ok(());
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
