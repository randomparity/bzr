use crate::cli::ServerAction;
use crate::commands::runtime::context::CommandContext;
use crate::error::Result;
use crate::output::resources::server::write_server_info;
use crate::output::writers::Writers;

pub async fn execute(
    action: &ServerAction,
    ctx: &CommandContext,
    w: &mut Writers<'_>,
) -> Result<()> {
    let client = super::runtime::shared::connect_and_configure(ctx).await?;

    match action {
        ServerAction::Info => {
            let info = client.server_info().await?;
            write_server_info(&info, ctx.format(), w.out);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
