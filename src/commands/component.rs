use crate::cli::ComponentAction;
use crate::client::BugzillaClient;
use crate::commands::runtime::context::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::resources::component::{write_component, write_components};
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::OutputFormat;
use crate::types::{CreateComponentParams, UpdateComponentParams};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonCreateComponent {
    product: Option<String>,
    name: Option<String>,
    description: Option<String>,
    default_assignee: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonUpdateComponent {
    id: Option<u64>,
    product: Option<String>,
    component: Option<String>,
    name: Option<String>,
    description: Option<String>,
    default_assignee: Option<String>,
}

enum ComponentUpdateTarget {
    Id(u64),
    Named { product: String, component: String },
}

struct ComponentUpdateInput {
    target: ComponentUpdateTarget,
    params: UpdateComponentParams,
}

struct UpdateParamSources<'a> {
    from_json: Option<&'a str>,
    id: Option<u64>,
    product: Option<&'a str>,
    component: Option<&'a str>,
    name: Option<&'a str>,
    description: Option<&'a str>,
    default_assignee: Option<&'a str>,
}

struct UpdateTargetSources<'a> {
    cli_id: Option<u64>,
    json_id: Option<u64>,
    cli_product: Option<&'a str>,
    cli_component: Option<&'a str>,
    json_product: Option<String>,
    json_component: Option<String>,
}

struct NamedComponentTarget {
    product: String,
    component: String,
}

pub async fn execute(
    action: &ComponentAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    match action {
        ComponentAction::List { product } => {
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            let product = client.get_product(product).await?;
            write_components(&product.components, format, w.out);
        }
        ComponentAction::View { product, name } => {
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            let product = client.get_product(product).await?;
            let component = product
                .components
                .iter()
                .find(|c| c.name == *name)
                .ok_or_else(|| BzrError::NotFound {
                    resource: "component",
                    id: name.clone(),
                })?;
            write_component(component, format, w.out);
        }
        ComponentAction::Create {
            from_json,
            product,
            name,
            description,
            default_assignee,
        } => {
            let params = build_create_params(
                from_json.as_deref(),
                product.as_deref(),
                name.as_deref(),
                description.as_deref(),
                default_assignee.as_deref(),
            )?;
            if ctx.dry_run() {
                let message = format!(
                    "Would create component '{}' in product '{}'",
                    params.name, params.product
                );
                write_result(
                    &DryRunResult::new(ResourceKind::Component, &[], &params),
                    &message,
                    format,
                    w.out,
                );
                return Ok(());
            }
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            let id = client.create_component(&params).await?;
            write_result(
                &ActionResult::created(id, ResourceKind::Component),
                &format!("Created component #{id} in product '{}'", params.product),
                format,
                w.out,
            );
        }
        ComponentAction::Update {
            from_json,
            id,
            product,
            component,
            name,
            description,
            default_assignee,
        } => {
            let sources = UpdateParamSources {
                from_json: from_json.as_deref(),
                id: *id,
                product: product.as_deref(),
                component: component.as_deref(),
                name: name.as_deref(),
                description: description.as_deref(),
                default_assignee: default_assignee.as_deref(),
            };
            execute_update(&sources, ctx, w).await?;
        }
    }
    Ok(())
}

async fn execute_update(
    sources: &UpdateParamSources<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    let update = build_update_input(sources)?;
    if ctx.dry_run() {
        if let ComponentUpdateTarget::Id(id) = &update.target {
            write_update_dry_run(*id, &update.params, format, w);
            return Ok(());
        }
    }
    let client = super::runtime::shared::connect_and_configure(ctx).await?;
    let id = resolve_update_target_id(&client, &update.target).await?;
    if ctx.dry_run() {
        write_update_dry_run(id, &update.params, format, w);
        return Ok(());
    }
    client.update_component(id, &update.params).await?;
    write_result(
        &ActionResult::updated(id, ResourceKind::Component),
        &format!("Updated component #{id}"),
        format,
        w.out,
    );
    Ok(())
}

fn write_update_dry_run(
    id: u64,
    params: &UpdateComponentParams,
    format: OutputFormat,
    w: &mut Writers<'_>,
) {
    let ids = [id];
    let message = format!("Would update component #{id}");
    write_result(
        &DryRunResult::new(ResourceKind::Component, &ids, params),
        &message,
        format,
        w.out,
    );
}

#[must_use]
pub fn is_dry_runnable(action: &ComponentAction) -> bool {
    matches!(
        action,
        ComponentAction::Create { .. } | ComponentAction::Update { .. }
    )
}

pub(crate) fn requires_credentials(action: &ComponentAction) -> Option<&'static str> {
    match action {
        ComponentAction::List { .. } | ComponentAction::View { .. } => None,
        ComponentAction::Create { .. } => Some("component create"),
        ComponentAction::Update { .. } => Some("component update"),
    }
}

fn build_create_params(
    from_json: Option<&str>,
    product: Option<&str>,
    name: Option<&str>,
    description: Option<&str>,
    default_assignee: Option<&str>,
) -> Result<CreateComponentParams> {
    let mut input = if let Some(arg) = from_json {
        super::runtime::from_json::read_object::<JsonCreateComponent>(arg)?
    } else {
        JsonCreateComponent::default()
    };
    super::runtime::from_json::merge_string(&mut input.product, product);
    super::runtime::from_json::merge_string(&mut input.name, name);
    super::runtime::from_json::merge_string(&mut input.description, description);
    super::runtime::from_json::merge_string(&mut input.default_assignee, default_assignee);
    Ok(CreateComponentParams {
        product: super::runtime::from_json::required_string(input.product, "product")?,
        name: super::runtime::from_json::required_string(input.name, "name")?,
        description: super::runtime::from_json::required_string(input.description, "description")?,
        default_assignee: super::runtime::from_json::required_string(
            input.default_assignee,
            "default_assignee",
        )?,
    })
}

fn build_update_input(sources: &UpdateParamSources<'_>) -> Result<ComponentUpdateInput> {
    let mut input = if let Some(arg) = sources.from_json {
        super::runtime::from_json::read_object::<JsonUpdateComponent>(arg)?
    } else {
        JsonUpdateComponent::default()
    };
    let target = resolve_update_target(UpdateTargetSources {
        cli_id: sources.id,
        json_id: input.id.take(),
        cli_product: sources.product,
        cli_component: sources.component,
        json_product: input.product.take(),
        json_component: input.component.take(),
    })?;
    super::runtime::from_json::merge_string(&mut input.name, sources.name);
    super::runtime::from_json::merge_string(&mut input.description, sources.description);
    super::runtime::from_json::merge_string(&mut input.default_assignee, sources.default_assignee);
    let params = UpdateComponentParams {
        name: input.name,
        description: input.description,
        default_assignee: input.default_assignee,
    };
    validate_update_params(&params)?;
    Ok(ComponentUpdateInput { target, params })
}

fn resolve_update_target(sources: UpdateTargetSources<'_>) -> Result<ComponentUpdateTarget> {
    if sources.cli_id.is_some() && sources.json_id.is_some() {
        return Err(BzrError::InputValidation(
            "--from-json object cannot combine positional component ID with JSON id".into(),
        ));
    }
    let cli_named = named_target_from_cli(sources.cli_product, sources.cli_component)?;
    let json_named = named_target_from_json(sources.json_product, sources.json_component)?;
    let id = sources.cli_id.or(sources.json_id);

    if id.is_some() && (cli_named.is_some() || json_named.is_some()) {
        return Err(BzrError::InputValidation(
            "component update target must use either component ID or --product/--component, \
             not both"
                .into(),
        ));
    }
    if cli_named.is_some() && json_named.is_some() {
        return Err(BzrError::InputValidation(
            "--from-json object cannot combine CLI --product/--component with JSON \
             product/component target fields"
                .into(),
        ));
    }
    if let Some(id) = id {
        return Ok(ComponentUpdateTarget::Id(id));
    }
    let named = cli_named.or(json_named).ok_or_else(|| {
        BzrError::InputValidation(
            "component update requires a component target: pass <ID>, \
             --product <PRODUCT> --component <COMPONENT>, or JSON id/product/component \
             via --from-json"
                .into(),
        )
    })?;
    Ok(ComponentUpdateTarget::Named {
        product: named.product,
        component: named.component,
    })
}

fn named_target_from_cli(
    product: Option<&str>,
    component: Option<&str>,
) -> Result<Option<NamedComponentTarget>> {
    match (product, component) {
        (Some(product), Some(component)) => Ok(Some(NamedComponentTarget {
            product: product.to_string(),
            component: component.to_string(),
        })),
        (Some(_), None) => Err(BzrError::InputValidation(
            "--product requires --component to target component update by name".into(),
        )),
        (None, Some(_)) => Err(BzrError::InputValidation(
            "--component requires --product to target component update by name".into(),
        )),
        (None, None) => Ok(None),
    }
}

fn named_target_from_json(
    product: Option<String>,
    component: Option<String>,
) -> Result<Option<NamedComponentTarget>> {
    match (product, component) {
        (Some(product), Some(component)) => Ok(Some(NamedComponentTarget { product, component })),
        (Some(_), None) | (None, Some(_)) => Err(BzrError::InputValidation(
            "--from-json: 'product' and 'component' must be supplied together for \
             name-based component update targeting"
                .into(),
        )),
        (None, None) => Ok(None),
    }
}

async fn resolve_update_target_id(
    client: &BugzillaClient,
    target: &ComponentUpdateTarget,
) -> Result<u64> {
    match target {
        ComponentUpdateTarget::Id(id) => Ok(*id),
        ComponentUpdateTarget::Named { product, component } => {
            let product_data = client.get_product(product).await?;
            find_component_id(&product_data, product, component)
        }
    }
}

fn find_component_id(
    product_data: &crate::types::Product,
    product: &str,
    component_name: &str,
) -> Result<u64> {
    let mut found = None;
    for component in &product_data.components {
        if component.name != component_name {
            continue;
        }
        if found.is_some() {
            return Err(BzrError::InputValidation(format!(
                "component name '{component_name}' is ambiguous in product '{product}'; \
                 use numeric component ID"
            )));
        }
        found = Some(component.id);
    }
    found.ok_or_else(|| BzrError::NotFound {
        resource: "component",
        id: format!("{product}/{component_name}"),
    })
}

fn validate_update_params(params: &UpdateComponentParams) -> Result<()> {
    if params.name.is_none() && params.description.is_none() && params.default_assignee.is_none() {
        return Err(BzrError::InputValidation(
            "no fields to update; specify at least one field to change".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "component_tests.rs"]
mod tests;
