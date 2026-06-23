use serde::Deserialize;

use crate::commands::runtime::context::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::user::UpdateUserParams;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonUpdateUser {
    user: Option<String>,
    real_name: Option<String>,
    email: Option<String>,
    disable_login: Option<bool>,
    login_denied_text: Option<String>,
}

pub(super) struct UpdateArgs<'a> {
    pub(super) from_json: Option<&'a str>,
    pub(super) user: Option<&'a str>,
    pub(super) real_name: Option<&'a str>,
    pub(super) email: Option<&'a str>,
    pub(super) disable_login: Option<bool>,
    pub(super) login_denied_text: Option<&'a str>,
}

pub(super) async fn handle(
    args: &UpdateArgs<'_>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let (user, params) = build_params(args)?;
    let format = ctx.format();
    if ctx.dry_run() {
        let message = format!("Would update user '{user}'");
        write_result(
            &DryRunResult::new(ResourceKind::User, &[], &params),
            &message,
            format,
            w.out,
        );
        return Ok(());
    }
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    client.update_user(&user, &params).await?;
    write_result(
        &ActionResult::updated_named(user.as_str(), None, ResourceKind::User),
        &format!("Updated user '{user}'"),
        format,
        w.out,
    );
    Ok(())
}

fn build_params(args: &UpdateArgs<'_>) -> Result<(String, UpdateUserParams)> {
    let mut input = if let Some(arg) = args.from_json {
        crate::commands::runtime::from_json::read_object::<JsonUpdateUser>(arg)?
    } else {
        JsonUpdateUser::default()
    };
    let target = crate::commands::runtime::from_json::resolve_string_target(
        args.user,
        input.user.take(),
        "--from-json object cannot combine positional user with JSON user",
        "--from-json object requires a user",
    )?;
    crate::commands::runtime::from_json::merge_string(&mut input.real_name, args.real_name);
    crate::commands::runtime::from_json::merge_string(&mut input.email, args.email);
    crate::commands::runtime::from_json::merge_copy(&mut input.disable_login, args.disable_login);
    crate::commands::runtime::from_json::merge_string(
        &mut input.login_denied_text,
        args.login_denied_text,
    );
    let denied_text =
        resolve_login_denied_text(input.disable_login, input.login_denied_text.as_deref());
    let params = UpdateUserParams {
        names: Some(vec![target.clone()]),
        real_name: input.real_name,
        email: input.email,
        login_denied_text: denied_text,
    };
    validate_params(&params)?;
    Ok((target, params))
}

/// Compute the Bugzilla `login_denied_text` field from CLI flags.
///
/// - `--disable-login` with custom text: use that text
/// - `--disable-login` without text: default "Account disabled"
/// - `--disable-login=false`: empty string, re-enabling login
/// - neither flag: `None`, leaving login state unchanged
pub(super) fn resolve_login_denied_text(
    disable: Option<bool>,
    custom_text: Option<&str>,
) -> Option<String> {
    match (disable, custom_text) {
        (Some(true), Some(text)) => Some(text.into()),
        (Some(true), None) => Some("Account disabled".into()),
        (Some(false), _) => Some(String::new()),
        (None, _) => None,
    }
}

fn validate_params(params: &UpdateUserParams) -> Result<()> {
    if params.real_name.is_none() && params.email.is_none() && params.login_denied_text.is_none() {
        return Err(BzrError::InputValidation(
            "no fields to update; specify at least one field to change".into(),
        ));
    }
    Ok(())
}
