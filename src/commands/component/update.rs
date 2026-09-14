use crate::commands::runtime::invocation::CommandContext;
use crate::commands::runtime::shared::{
    connect_and_configure, require_server_capability, RED_HAT_EXTENSION,
};
use crate::error::{BzrError, Result};
use crate::output::result_types::write_result;
use crate::output::writers::Writers;
use crate::types::component::UpdateComponentParams;

pub(super) struct UpdateArgs<'a> {
    pub(super) product: &'a str,
    pub(super) component: &'a str,
    pub(super) description: Option<&'a str>,
    pub(super) default_assignee: Option<&'a str>,
    pub(super) is_active: Option<bool>,
}

pub(super) async fn handle(
    args: &UpdateArgs<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    validate(args)?;
    let params = UpdateComponentParams {
        product: args.product,
        component: args.component,
        description: args.description,
        default_assignee: args.default_assignee,
        is_active: args.is_active,
    };
    if ctx.dry_run() {
        write_result(
            &serde_json::json!({
                "resource": "component",
                "action": "dry-run",
                "product": params.product,
                "component": params.component,
                "changes": {
                    "description": params.description,
                    "default_assignee": params.default_assignee,
                    "is_active": params.is_active,
                },
            }),
            &format!(
                "Would update component '{}' in product '{}'",
                params.component, params.product
            ),
            ctx.format(),
            w.out,
        );
        return Ok(());
    }
    let client = connect_and_configure(ctx).await?;
    if client.auth_mode() == crate::types::AuthMode::Token {
        return Err(BzrError::Auth(
            "component update uses XML-RPC and requires an API key; login tokens support REST only"
                .into(),
        ));
    }
    require_server_capability(ctx, &client, RED_HAT_EXTENSION, "component update").await?;
    client.update_component(params).await?;
    write_result(
        &serde_json::json!({
            "resource": "component",
            "action": "updated",
            "product": args.product,
            "component": args.component,
        }),
        &format!(
            "Updated component '{}' in product '{}'",
            args.component, args.product
        ),
        ctx.format(),
        w.out,
    );
    Ok(())
}

fn validate(args: &UpdateArgs<'_>) -> Result<()> {
    for (name, value) in [("--product", args.product), ("--component", args.component)] {
        if value.trim().is_empty() {
            return Err(BzrError::input(format!("{name} must not be empty")));
        }
    }
    if args.description.is_none() && args.default_assignee.is_none() && args.is_active.is_none() {
        return Err(BzrError::input(
            "no fields to update; specify --description, --default-assignee, or --is-active".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
