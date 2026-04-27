use clap::Subcommand;

#[derive(Subcommand)]
pub enum QueryAction {
    /// Save a named query
    Save {
        /// Query name
        name: String,
        /// Import query from a Bugzilla buglist.cgi URL (mutually exclusive with filter flags)
        #[arg(long, conflicts_with_all = ["search", "product", "component", "status", "assignee", "creator", "priority", "severity"])]
        from_url: Option<String>,
        /// Free-text search (creates a "search" kind query)
        #[arg(long)]
        search: Option<String>,
        /// Filter by product (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        product: Vec<String>,
        /// Filter by component (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        component: Vec<String>,
        /// Filter by status (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        status: Vec<String>,
        /// Filter by assignee (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        assignee: Vec<String>,
        /// Filter by creator (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        creator: Vec<String>,
        /// Filter by priority (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        priority: Vec<String>,
        /// Filter by severity (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        severity: Vec<String>,
        /// Max number of results
        #[arg(long)]
        limit: Option<u32>,
        /// Only return these fields (comma-separated)
        #[arg(long)]
        fields: Option<String>,
        /// Exclude these fields (comma-separated)
        #[arg(long)]
        exclude_fields: Option<String>,
    },
    /// List all saved queries
    List,
    /// Show details of a saved query
    Show {
        /// Query name
        name: String,
    },
    /// Delete a saved query
    Delete {
        /// Query name
        name: String,
    },
    /// Run a saved query
    Run {
        /// Query name
        name: String,
        /// Override the saved limit
        #[arg(long)]
        limit: Option<u32>,
        /// Override the saved fields selection
        #[arg(long)]
        fields: Option<String>,
        /// Override the saved exclude-fields selection
        #[arg(long)]
        exclude_fields: Option<String>,
        /// Override the server to run against
        #[arg(long)]
        server: Option<String>,
    },
}
