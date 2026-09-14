use crate::cli::{
    AddExternalBugArgs, ExternalBugAction, ExternalBugArgs, RemoveExternalBugArgs,
    UpdateExternalBugArgs,
};
use crate::commands::runtime::invocation::CommandContext;
use crate::commands::runtime::shared::{
    connect_and_configure, require_server_capability, EXTERNAL_BUGS_EXTENSION,
};
use crate::error::{BzrError, Result};
use crate::output::result_types::write_result;
use crate::output::writers::Writers;
use crate::types::bug::ExternalBugMutation;

fn require_text(value: &str, flag: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(BzrError::input(format!("{flag} must not be empty")));
    }
    Ok(())
}

fn validate(action: &ExternalBugAction) -> Result<()> {
    match action {
        ExternalBugAction::Add(args) => {
            require_text(&args.external_id, "--external-id")?;
            require_text(&args.status, "--status")?;
            require_text(&args.description, "--description")
        }
        ExternalBugAction::Update(args) => {
            require_text(&args.external_id, "--external-id")?;
            require_text(&args.status, "--status")?;
            require_text(&args.description, "--description")
        }
        ExternalBugAction::Remove(args) => require_text(&args.external_id, "--external-id"),
    }
}

pub(super) async fn handle(
    args: &ExternalBugArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    validate(&args.action)?;
    if ctx.dry_run() {
        write_result(
            &serde_json::json!({"resource": "external_bug", "action": "dry-run", "changes": format!("{:?}", args.action)}),
            "Would mutate an external bug link",
            ctx.format(),
            w.out,
        );
        return Ok(());
    }
    let client = connect_and_configure(ctx).await?;
    if client.auth_mode() == crate::types::AuthMode::Token {
        return Err(BzrError::Auth(
            "bug external-bug uses XML-RPC and requires an API key; login tokens support REST only"
                .into(),
        ));
    }
    require_server_capability(
        ctx,
        &client,
        EXTERNAL_BUGS_EXTENSION,
        "ExternalBugs mutation",
    )
    .await?;
    match &args.action {
        ExternalBugAction::Add(AddExternalBugArgs {
            id,
            tracker,
            external_id,
            status,
            description,
        }) => {
            client
                .add_external_bug(ExternalBugMutation {
                    bug_id: *id,
                    tracker_id: *tracker,
                    external_id,
                    status: Some(status),
                    description: Some(description),
                })
                .await?;
            write_result(
                &serde_json::json!({"bug_id": id, "tracker_id": tracker, "external_id": external_id, "resource": "external_bug", "action": "created"}),
                &format!("Added external bug {external_id} to bug #{id}"),
                ctx.format(),
                w.out,
            );
        }
        ExternalBugAction::Update(UpdateExternalBugArgs {
            id,
            tracker,
            external_id,
            status,
            description,
        }) => {
            client
                .update_external_bug(ExternalBugMutation {
                    bug_id: *id,
                    tracker_id: *tracker,
                    external_id,
                    status: Some(status),
                    description: Some(description),
                })
                .await?;
            write_result(
                &serde_json::json!({"bug_id": id, "tracker_id": tracker, "external_id": external_id, "resource": "external_bug", "action": "updated"}),
                &format!("Updated external bug {external_id} on bug #{id}"),
                ctx.format(),
                w.out,
            );
        }
        ExternalBugAction::Remove(RemoveExternalBugArgs {
            id,
            tracker,
            external_id,
        }) => {
            client
                .remove_external_bug(ExternalBugMutation {
                    bug_id: *id,
                    tracker_id: *tracker,
                    external_id,
                    status: None,
                    description: None,
                })
                .await?;
            write_result(
                &serde_json::json!({"bug_id": id, "tracker_id": tracker, "external_id": external_id, "resource": "external_bug", "action": "removed"}),
                &format!("Removed external bug {external_id} from bug #{id}"),
                ctx.format(),
                w.out,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "external_bug_tests.rs"]
mod tests;
