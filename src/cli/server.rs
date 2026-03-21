use clap::Subcommand;

#[derive(Subcommand)]
pub enum ServerAction {
    /// Show server version and extensions
    Info,
}
