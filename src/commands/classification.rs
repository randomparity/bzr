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
    let client = super::runtime::shared::connect_and_configure(ctx).await?;

    match action {
        ClassificationAction::List => {
            let classifications = client.list_classifications().await?;
            // A lone "Unclassified" is Bugzilla's signal that classifications
            // are disabled on this server.
            let disabled = matches!(
                classifications.as_slice(),
                [only] if only.name.eq_ignore_ascii_case("Unclassified")
            );
            if disabled {
                let _ = writeln!(
                    w.err,
                    "Note: only the default 'Unclassified' classification exists; \
                     this server likely has classifications disabled."
                );
            }
            write_classifications(&classifications, format, w.out);
        }
        ClassificationAction::View { name } => {
            let classification = client.get_classification(name).await?;
            write_classification(&classification, format, w.out);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "classification_tests.rs"]
mod tests;
