use crate::cli::UserAction;
use crate::error::Result;
use crate::output::resources::user::{write_users, write_users_detailed};
use crate::output::result_types::{write_result, ActionResult, ResourceKind};
use crate::output::writers::Writers;
use crate::types::ApiMode;
use crate::types::OutputFormat;
use crate::types::{CreateUserParams, UpdateUserParams};

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

pub async fn execute(
    action: &UserAction,
    server: Option<&str>,
    format: OutputFormat,
    api: Option<ApiMode>,
    w: &mut Writers<'_>,
) -> Result<()> {
    let client = super::runtime::shared::connect_and_configure(server, api).await?;

    match action {
        UserAction::Search { query, details } => {
            let users = client.search_users(query, *details).await?;
            if *details {
                write_users_detailed(&users, format, w.out);
            } else {
                write_users(&users, format, w.out);
            }
        }
        UserAction::Create {
            email,
            login,
            full_name,
            password,
        } => {
            let params = CreateUserParams {
                email: email.clone(),
                login: login.clone(),
                full_name: full_name.clone(),
                password: password.clone(),
            };
            let id = client.create_user(&params).await?;
            write_result(
                &ActionResult::created_named(id, email.as_str(), ResourceKind::User),
                &format!("Created user #{id} ({email})"),
                format,
                w.out,
            );
        }
        UserAction::Update {
            user,
            real_name,
            email,
            disable_login,
            login_denied_text,
        } => {
            let denied_text =
                resolve_login_denied_text(*disable_login, login_denied_text.as_deref());
            let params = UpdateUserParams {
                names: Some(vec![user.clone()]),
                real_name: real_name.clone(),
                email: email.clone(),
                login_denied_text: denied_text,
            };
            client.update_user(user, &params).await?;
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

#[cfg(test)]
#[path = "user_tests.rs"]
mod tests;
