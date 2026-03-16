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
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else {
        // Level strings are compile-time constants; parse_lossy is safe.
        let level = match cli.verbose {
            0 => "bzr=warn",
            1 => "bzr=info",
            2 => "bzr=debug",
            _ => "bzr=trace",
        };
        EnvFilter::new(level)
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

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
