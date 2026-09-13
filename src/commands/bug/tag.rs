use crate::cli::TagArgs;
use crate::commands::runtime::invocation::CommandContext;
use crate::error::{BzrError, Result};
use crate::output::result_types::write_result;
use crate::output::writers::Writers;

pub(super) async fn handle(
    args: &TagArgs,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    if args.add.is_empty() && args.remove.is_empty() {
        return Err(BzrError::input(
            "no bug tag changes; specify --add or --remove".into(),
        ));
    }
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    client
        .update_bug_tags(args.id, &args.add, &args.remove)
        .await?;
    write_result(
        &serde_json::json!({"bug_id": args.id, "added": args.add, "removed": args.remove, "resource": "bug", "action": "updated"}),
        &format!("Updated tags on bug #{}", args.id),
        ctx.format(),
        w.out,
    );
    Ok(())
}

#[cfg(test)]
#[path = "tag_tests.rs"]
mod tests;
