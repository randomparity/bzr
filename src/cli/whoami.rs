use clap::Subcommand;

/// Whoami action — shows the authenticated user's identity.
#[derive(Subcommand)]
pub enum WhoamiAction {
    /// Show the currently authenticated user
    Show,
}
