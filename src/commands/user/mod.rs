//! User subcommand handlers, split per action.

use crate::cli::UserAction;
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::writers::Writers;

mod create;
mod search;
mod update;

pub async fn execute(action: &UserAction, ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    match action {
        UserAction::Search { query, details } => search::handle(query, *details, ctx, w).await,
        UserAction::Create {
            from_json,
            email,
            login,
            full_name,
            password,
        } => {
            create::handle(
                create::CreateArgs {
                    from_json: from_json.as_deref(),
                    email: email.as_deref(),
                    login: login.as_deref(),
                    full_name: full_name.as_deref(),
                    password: password.as_deref(),
                },
                ctx,
                w,
            )
            .await
        }
        UserAction::Update {
            from_json,
            user,
            real_name,
            email,
            disable_login,
            login_denied_text,
        } => {
            update::handle(
                from_json.as_deref(),
                user.as_deref(),
                update::UserUpdateCli {
                    real_name: real_name.as_deref(),
                    email: email.as_deref(),
                    disable_login: *disable_login,
                    login_denied_text: login_denied_text.as_deref(),
                },
                ctx,
                w,
            )
            .await
        }
    }
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

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
