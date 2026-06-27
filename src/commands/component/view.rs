use crate::commands::runtime::context::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::resources::component::write_component;
use crate::output::writers::Writers;

pub(super) async fn handle(
    product: &str,
    name: &str,
    projection_args: &crate::cli::ProjectionArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let projection = crate::validation::fields::projection_for(
        ctx.format(),
        projection_args.fields.as_deref(),
        projection_args.exclude_fields.as_deref(),
        crate::types::component::COMPONENT_FIELDS,
        w.err,
    )?;
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let product = client.get_product(product).await?;
    let component = product
        .components
        .iter()
        .find(|c| c.name.as_deref() == Some(name))
        .ok_or_else(|| BzrError::NotFound {
            resource: "component",
            id: name.to_owned(),
        })?;
    write_component(component, ctx.format(), &projection, w.out);
    Ok(())
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
