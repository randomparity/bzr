use clap::Subcommand;

#[derive(Subcommand)]
pub enum FieldAction {
    /// Show available field name aliases
    Aliases,
    /// List valid values for a bug field
    List {
        /// Field name (e.g. status, priority, severity, resolution).
        /// Common aliases are resolved automatically (status -> `bug_status`,
        /// severity -> `bug_severity`, etc.)
        name: String,
    },
}
