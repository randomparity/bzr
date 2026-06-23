use crate::client::BugzillaClient;
use crate::commands::runtime::context::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::common::OutputFormat;
use crate::types::component::UpdateComponentParams;
use crate::types::product::Product;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize)]
struct NamedComponentUpdateDryRun<'a> {
    product: &'a str,
    component: &'a str,
    #[serde(flatten)]
    params: &'a UpdateComponentParams,
}

pub(super) struct UpdateArgs<'a> {
    pub(super) from_json: Option<&'a str>,
    pub(super) id: Option<u64>,
    pub(super) product: Option<&'a str>,
    pub(super) component: Option<&'a str>,
    pub(super) name: Option<&'a str>,
    pub(super) description: Option<&'a str>,
    pub(super) default_assignee: Option<&'a str>,
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

pub(super) async fn handle(
    args: &UpdateArgs<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let format = ctx.format();
    let update = build_update_input(args)?;
    if ctx.dry_run() {
        write_update_dry_run(&update, format, w);
        return Ok(());
    }
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let id = resolve_update_target_id(&client, &update.target).await?;
    client.update_component(id, &update.params).await?;
    write_result(
        &ActionResult::updated(id, ResourceKind::Component),
        &format!("Updated component #{id}"),
        format,
        w.out,
    );
    Ok(())
}

fn write_update_dry_run(update: &ComponentUpdateInput, format: OutputFormat, w: &mut Writers<'_>) {
    match &update.target {
        ComponentUpdateTarget::Id(id) => {
            let ids = [*id];
            let message = format!("Would update component #{id}");
            write_result(
                &DryRunResult::new(ResourceKind::Component, &ids, &update.params),
                &message,
                format,
                w.out,
            );
        }
        ComponentUpdateTarget::Named { product, component } => {
            let message = format!("Would update component '{component}' in product '{product}'");
            let payload = NamedComponentUpdateDryRun {
                product,
                component,
                params: &update.params,
            };
            write_result(
                &DryRunResult::new(ResourceKind::Component, &[], &payload),
                &message,
                format,
                w.out,
            );
        }
    }
}

fn build_update_input(args: &UpdateArgs<'_>) -> Result<ComponentUpdateInput> {
    let mut input = if let Some(arg) = args.from_json {
        crate::commands::runtime::from_json::read_object::<JsonUpdateComponent>(arg)?
    } else {
        JsonUpdateComponent::default()
    };
    let target = resolve_update_target(UpdateTargetSources {
        cli_id: args.id,
        json_id: input.id.take(),
        cli_product: args.product,
        cli_component: args.component,
        json_product: input.product.take(),
        json_component: input.component.take(),
    })?;
    crate::commands::runtime::from_json::merge_string(&mut input.name, args.name);
    crate::commands::runtime::from_json::merge_string(&mut input.description, args.description);
    crate::commands::runtime::from_json::merge_string(
        &mut input.default_assignee,
        args.default_assignee,
    );
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

fn find_component_id(product_data: &Product, product: &str, component_name: &str) -> Result<u64> {
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
