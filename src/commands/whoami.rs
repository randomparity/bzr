//! Whoami command — shows the authenticated user's identity.

use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::user::write_whoami;
use crate::output::writers::Writers;

pub async fn execute(ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let client = super::runtime::shared::connect_and_configure(ctx).await?;
    let whoami = client.whoami().await?;
    write_whoami(&whoami, ctx.format(), w.out);
    Ok(())
}

#[cfg(test)]
#[path = "whoami_tests.rs"]
mod tests;
