mod auth;
mod cli;
mod client;
mod commands;
mod config;
mod error;
#[expect(clippy::print_stdout, clippy::unwrap_used)]
mod output;

use clap::Parser;
use cli::{Cli, Commands};
use output::OutputFormat;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        #[expect(clippy::print_stderr)]
        {
            eprintln!("error: {e}");
        }
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> error::Result<()> {
    let format: OutputFormat = cli
        .output
        .parse()
        .map_err(|e: String| error::BzrError::Other(e))?;

    match &cli.command {
        Commands::Bug { action } => {
            commands::bug::execute(action, cli.server.as_deref(), format).await
        }
        Commands::Comment { action } => {
            commands::comment::execute(action, cli.server.as_deref(), format).await
        }
        Commands::Attachment { action } => {
            commands::attachment::execute(action, cli.server.as_deref(), format).await
        }
        Commands::Config { action } => commands::config_cmd::execute(action),
    }
}
