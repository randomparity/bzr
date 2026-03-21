use clap::Subcommand;

#[derive(Subcommand)]
pub enum ClassificationAction {
    /// View a classification by name or ID
    View {
        /// Classification name or ID
        name: String,
    },
}
