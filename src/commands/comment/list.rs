use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::comment::write_comments;
use crate::output::writers::Writers;

pub(super) async fn handle(
    bug_id: u64,
    since: Option<&str>,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let canonical_since = crate::validation::parse_optional_date(since, "--since")?;
    let client = crate::commands::runtime::shared::connect_and_configure(ctx).await?;
    let comments = client
        .get_comments_since(bug_id, canonical_since.as_deref())
        .await?;
    write_comments(&comments, ctx.format(), w.out);
    Ok(())
}
