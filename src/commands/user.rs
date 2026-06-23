use crate::cli::UserAction;
use crate::commands::runtime::context::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::resources::user::{write_users, write_users_detailed};
use crate::output::result_types::{write_result, ActionResult, DryRunResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::{CreateUserParams, UpdateUserParams};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonCreateUser {
    email: Option<String>,
    login: Option<String>,
    full_name: Option<String>,
    password: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsonUpdateUser {
    user: Option<String>,
    real_name: Option<String>,
    email: Option<String>,
    disable_login: Option<bool>,
    login_denied_text: Option<String>,
}

#[derive(Clone, Copy)]
struct UserUpdateCli<'a> {
    real_name: Option<&'a str>,
    email: Option<&'a str>,
    disable_login: Option<bool>,
    login_denied_text: Option<&'a str>,
}

/// Compute the Bugzilla `login_denied_text` field from CLI flags.
///
/// - `--disable-login` with custom text → use that text
/// - `--disable-login` without text → default "Account disabled"
/// - `--disable-login=false` → empty string (re-enables login)
/// - neither flag → `None` (leave unchanged)
fn resolve_login_denied_text(disable: Option<bool>, custom_text: Option<&str>) -> Option<String> {
    match (disable, custom_text) {
        (Some(true), Some(text)) => Some(text.into()),
        (Some(true), None) => Some("Account disabled".into()),
        (Some(false), _) => Some(String::new()),
        (None, _) => None,
    }
}

pub async fn execute(action: &UserAction, ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let format = ctx.format();
    match action {
        UserAction::Search { query, details } => {
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            let users = client.search_users(query, *details).await?;
            if *details {
                write_users_detailed(&users, format, w.out);
            } else {
                write_users(&users, format, w.out);
            }
        }
        UserAction::Create {
            from_json,
            email,
            login,
            full_name,
            password,
        } => {
            let params = build_create_params(
                from_json.as_deref(),
                email.as_deref(),
                login.as_deref(),
                full_name.as_deref(),
                password.as_deref(),
            )?;
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
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            let id = client.create_user(&params).await?;
            write_result(
                &ActionResult::created_named(id, params.email.as_str(), ResourceKind::User),
                &format!("Created user #{id} ({})", params.email),
                format,
                w.out,
            );
        }
        UserAction::Update {
            from_json,
            user,
            real_name,
            email,
            disable_login,
            login_denied_text,
        } => {
            let (user, params) = build_update_params(
                from_json.as_deref(),
                user.as_deref(),
                UserUpdateCli {
                    real_name: real_name.as_deref(),
                    email: email.as_deref(),
                    disable_login: *disable_login,
                    login_denied_text: login_denied_text.as_deref(),
                },
            )?;
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
            let client = super::runtime::shared::connect_and_configure(ctx).await?;
            client.update_user(&user, &params).await?;
            write_result(
                &ActionResult::updated_named(user.as_str(), None, ResourceKind::User),
                &format!("Updated user '{user}'"),
                format,
                w.out,
            );
        }
    }
    Ok(())
}

#[must_use]
pub fn is_dry_runnable(action: &UserAction) -> bool {
    matches!(
        action,
        UserAction::Create { .. } | UserAction::Update { .. }
    )
}

pub(crate) fn requires_credentials(action: &UserAction) -> Option<&'static str> {
    match action {
        UserAction::Search { .. } => None,
        UserAction::Create { .. } => Some("user create"),
        UserAction::Update { .. } => Some("user update"),
    }
}

fn build_create_params(
    from_json: Option<&str>,
    email: Option<&str>,
    login: Option<&str>,
    full_name: Option<&str>,
    password: Option<&str>,
) -> Result<CreateUserParams> {
    let mut input = if let Some(arg) = from_json {
        super::runtime::from_json::read_object::<JsonCreateUser>(arg)?
    } else {
        JsonCreateUser::default()
    };
    super::runtime::from_json::merge_string(&mut input.email, email);
    super::runtime::from_json::merge_string(&mut input.login, login);
    super::runtime::from_json::merge_string(&mut input.full_name, full_name);
    super::runtime::from_json::merge_string(&mut input.password, password);
    Ok(CreateUserParams {
        email: super::runtime::from_json::required_string(input.email, "email")?,
        login: input.login,
        full_name: input.full_name,
        password: input.password,
    })
}

fn build_update_params(
    from_json: Option<&str>,
    user: Option<&str>,
    cli: UserUpdateCli<'_>,
) -> Result<(String, UpdateUserParams)> {
    let mut input = if let Some(arg) = from_json {
        super::runtime::from_json::read_object::<JsonUpdateUser>(arg)?
    } else {
        JsonUpdateUser::default()
    };
    let target = super::runtime::from_json::resolve_string_target(
        user,
        input.user.take(),
        "--from-json object cannot combine positional user with JSON user",
        "--from-json object requires a user",
    )?;
    super::runtime::from_json::merge_string(&mut input.real_name, cli.real_name);
    super::runtime::from_json::merge_string(&mut input.email, cli.email);
    super::runtime::from_json::merge_copy(&mut input.disable_login, cli.disable_login);
    super::runtime::from_json::merge_string(&mut input.login_denied_text, cli.login_denied_text);
    let denied_text =
        resolve_login_denied_text(input.disable_login, input.login_denied_text.as_deref());
    let params = UpdateUserParams {
        names: Some(vec![target.clone()]),
        real_name: input.real_name,
        email: input.email,
        login_denied_text: denied_text,
    };
    validate_update_params(&params)?;
    Ok((target, params))
}

fn validate_update_params(params: &UpdateUserParams) -> Result<()> {
    if params.real_name.is_none() && params.email.is_none() && params.login_denied_text.is_none() {
        return Err(BzrError::InputValidation(
            "no fields to update; specify at least one field to change".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "user_tests.rs"]
mod tests;
