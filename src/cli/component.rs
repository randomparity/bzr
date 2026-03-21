use clap::Subcommand;

#[derive(Subcommand)]
pub enum ComponentAction {
    /// Create a new component
    Create {
        /// Product name
        #[arg(long)]
        product: String,
        /// Component name
        #[arg(long)]
        name: String,
        /// Component description
        #[arg(long)]
        description: String,
        /// Default assignee email
        #[arg(long)]
        default_assignee: String,
    },
    /// Update a component
    Update {
        /// Component ID
        id: u64,
        /// New name
        #[arg(long)]
        name: Option<String>,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// New default assignee
        #[arg(long)]
        default_assignee: Option<String>,
    },
}
