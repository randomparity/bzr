use serde::Deserialize;

use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::CreateUserParams;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonCreateUser {
    email: Option<String>,
    login: Option<String>,
    full_name: Option<String>,
    password: Option<String>,
}

pub(super) struct CreateArgs<'a> {
    pub(super) from_json: Option<&'a str>,
    pub(super) email: Option<&'a str>,
    pub(super) login: Option<&'a str>,
    pub(super) full_name: Option<&'a str>,
    pub(super) password: Option<&'a str>,
}

pub(super) async fn handle(
    args: CreateArgs<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let params = build_params(
        args.from_json,
        args.email,
        args.login,
        args.full_name,
        args.password,
    )?;
    let format = ctx.format();
    if ctx.dry_run() {
        let message = format!("Would create user '{}'", params.email);
        write_result(
            &DryRunResult::new(ResourceKind::User, &[], &params),
            &message,
            format,
            w.out,
        );
        return Ok(());
    }
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let id = client.create_user(&params).await?;
    write_result(
        &ActionResult::created_named(id, params.email.as_str(), ResourceKind::User),
        &format!("Created user #{id} ({})", params.email),
        format,
        w.out,
    );
    Ok(())
}

fn build_params(
    from_json: Option<&str>,
    email: Option<&str>,
    login: Option<&str>,
    full_name: Option<&str>,
    password: Option<&str>,
) -> Result<CreateUserParams> {
    let mut input = if let Some(arg) = from_json {
        crate::commands::runtime::from_json::read_object::<JsonCreateUser>(arg)?
    } else {
        JsonCreateUser::default()
    };
    crate::commands::runtime::from_json::merge_string(&mut input.email, email);
    crate::commands::runtime::from_json::merge_string(&mut input.login, login);
    crate::commands::runtime::from_json::merge_string(&mut input.full_name, full_name);
    crate::commands::runtime::from_json::merge_string(&mut input.password, password);
    Ok(CreateUserParams {
        email: crate::commands::runtime::from_json::required_string(input.email, "email")?,
        login: input.login,
        full_name: input.full_name,
        password: input.password,
    })
}
