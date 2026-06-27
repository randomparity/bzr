use serde::Deserialize;

use crate::commands::runtime::invocation::CommandContext;
use crate::commands::runtime::mutation::{self, Committed, DryRunPreview};
use crate::error::Result;
use crate::output::result_types::{ActionResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::user::CreateUserParams;

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
    args: &CreateArgs<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let params = build_params(args)?;
    let message = format!("Would create user '{}'", params.email);
    mutation::run(
        ctx,
        w,
        DryRunPreview {
            resource: ResourceKind::User,
            params,
            message,
        },
        |client, params| async move {
            let id = client.create_user(&params).await?;
            Ok(Committed {
                result: ActionResult::created_named(id, params.email.as_str(), ResourceKind::User),
                message: format!("Created user #{id} ({})", params.email),
            })
        },
    )
    .await
}

fn build_params(args: &CreateArgs<'_>) -> Result<CreateUserParams> {
    let mut input = if let Some(arg) = args.from_json {
        crate::commands::runtime::input::from_json::read_object::<JsonCreateUser>(arg)?
    } else {
        JsonCreateUser::default()
    };
    crate::commands::runtime::input::from_json::merge_string(&mut input.email, args.email);
    crate::commands::runtime::input::from_json::merge_string(&mut input.login, args.login);
    crate::commands::runtime::input::from_json::merge_string(&mut input.full_name, args.full_name);
    crate::commands::runtime::input::from_json::merge_string(&mut input.password, args.password);
    Ok(CreateUserParams {
        email: crate::commands::runtime::input::from_json::required_string(input.email, "email")?,
        login: input.login,
        full_name: input.full_name,
        password: input.password,
    })
}

#[cfg(test)]
#[path = "create_tests.rs"]
mod tests;
