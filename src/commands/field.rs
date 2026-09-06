use crate::cli::FieldAction;
use crate::commands::runtime::invocation::CommandContext;
use crate::error::Result;
use crate::output::resources::field::{write_field_aliases, write_field_names, write_field_values};
use crate::output::writers::Writers;
use crate::types::{OutputFormat, FIELD_ALIASES};

pub(crate) async fn execute(
    action: &FieldAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    match action {
        FieldAction::Aliases => {
            write_field_aliases(FIELD_ALIASES, format, w.table_width(), w.out);
            return Ok(());
        }
        FieldAction::List {
            name: Some(name),
            projection,
        } => {
            let projection = crate::validation::fields::projection_for(
                format,
                projection.fields.as_deref(),
                projection.exclude_fields.as_deref(),
                crate::types::field::FIELD_VALUE_FIELDS,
                w.err,
            )?;
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            let values = client.get_field_values(name).await?;
            if values.is_empty() && format == OutputFormat::Table {
                let _ = writeln!(w.out, "No values for field '{name}'.");
            } else {
                write_field_values(&values, format, &projection, w.table_width(), w.out);
            }
        }
        FieldAction::List {
            name: None,
            projection,
        } => {
            let projection = crate::validation::fields::projection_for(
                format,
                projection.fields.as_deref(),
                projection.exclude_fields.as_deref(),
                crate::types::field::FIELD_NAME_FIELDS,
                w.err,
            )?;
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            // Always a fresh probe: `ServerConfig.bug_field_names` is a
            // validator fast path whose staleness is harmless there but would
            // make a listing disagree with the server (ADR 0062).
            let names = super::runtime::shared::accepted_bug_fields(&client).await?;
            write_field_names(&names, format, &projection, w.table_width(), w.out);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
