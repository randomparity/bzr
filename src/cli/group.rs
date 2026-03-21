use clap::Subcommand;

#[derive(Subcommand)]
pub enum GroupAction {
    /// Add a user to a group
    AddUser {
        /// Group name
        #[arg(long)]
        group: String,
        /// User email/login
        #[arg(long)]
        user: String,
    },
    /// Remove a user from a group
    RemoveUser {
        /// Group name
        #[arg(long)]
        group: String,
        /// User email/login
        #[arg(long)]
        user: String,
    },
    /// List users in a group
    ListUsers {
        /// Group name
        #[arg(long)]
        group: String,
        /// Show extended details (groups, login status)
        #[arg(long)]
        details: bool,
    },
    /// View group details
    View {
        /// Group name or ID
        group: String,
    },
    /// Create a new group
    Create {
        /// Group name
        #[arg(long)]
        name: String,
        /// Group description
        #[arg(long)]
        description: String,
        /// Whether the group is active
        #[arg(long, default_value = "true")]
        is_active: bool,
    },
    /// Update a group
    Update {
        /// Group name or ID
        group: String,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// Whether the group is active
        #[arg(long)]
        is_active: Option<bool>,
    },
}
