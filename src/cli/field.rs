use clap::Subcommand;

#[derive(Subcommand)]
pub enum FieldAction {
    /// List valid values for a bug field
    List {
        /// Field name (e.g. status, priority, severity, resolution)
        name: String,
    },
}
