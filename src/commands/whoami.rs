//! Whoami command — shows the authenticated user's identity.

use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::user::write_whoami;
use crate::output::writers::Writers;
use crate::types::user::WhoamiOutput;

pub async fn execute(ctx: &CommandContext, w: &mut Writers<'_>) -> Result<()> {
    let client = super::runtime::shared::connect_and_configure(ctx).await?;
    let identity = client.whoami().await?;
    let output = WhoamiOutput {
        identity,
        server_name: client.server_name().to_string(),
        auth_mode: client.auth_mode(),
    };
    write_whoami(&output, ctx.format(), w.out);
    Ok(())
}

#[cfg(test)]
#[path = "whoami_tests.rs"]
mod tests;
