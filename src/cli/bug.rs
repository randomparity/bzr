use clap::Subcommand;

#[derive(Subcommand)]
pub enum BugAction {
    /// List bugs with filters
    List {
        /// Filter by product
        #[arg(long)]
        product: Option<String>,
        /// Filter by component
        #[arg(long)]
        component: Option<String>,
        /// Filter by status
        #[arg(long)]
        status: Option<String>,
        /// Filter by assignee
        #[arg(long)]
        assignee: Option<String>,
        /// Filter by creator
        #[arg(long)]
        creator: Option<String>,
        /// Filter by priority
        #[arg(long)]
        priority: Option<String>,
        /// Filter by severity
        #[arg(long)]
        severity: Option<String>,
        /// Filter by bug IDs
        #[arg(long)]
        id: Vec<u64>,
        /// Filter by alias
        #[arg(long)]
        alias: Option<String>,
        /// Max number of results
        #[arg(long, default_value = "50")]
        limit: u32,
        /// Only return these fields (comma-separated)
        #[arg(long)]
        fields: Option<String>,
        /// Exclude these fields (comma-separated)
        #[arg(long)]
        exclude_fields: Option<String>,
    },
    /// View a bug by ID
    View {
        /// Bug ID or alias
        id: String,
        /// Only return these fields (comma-separated)
        #[arg(long)]
        fields: Option<String>,
        /// Exclude these fields (comma-separated)
        #[arg(long)]
        exclude_fields: Option<String>,
    },
    /// Search bugs by text query
    Search {
        /// Search query
        query: String,
        /// Max number of results
        #[arg(long, default_value = "50")]
        limit: u32,
        /// Only return these fields (comma-separated)
        #[arg(long)]
        fields: Option<String>,
        /// Exclude these fields (comma-separated)
        #[arg(long)]
        exclude_fields: Option<String>,
    },
    /// Show change history of a bug
    History {
        /// Bug ID
        id: u64,
        /// Only show changes after this date (ISO 8601)
        #[arg(long)]
        since: Option<String>,
    },
    /// Create a new bug
    Create {
        /// Use a saved template for default field values
        #[arg(long)]
        template: Option<String>,
        /// Product name (required unless provided by template)
        #[arg(long)]
        product: Option<String>,
        /// Component name (required unless provided by template)
        #[arg(long)]
        component: Option<String>,
        /// Bug summary
        #[arg(long)]
        summary: String,
        /// Version
        #[arg(long)]
        version: Option<String>,
        /// Bug description
        #[arg(long)]
        description: Option<String>,
        /// Priority
        #[arg(long)]
        priority: Option<String>,
        /// Severity
        #[arg(long)]
        severity: Option<String>,
        /// Assignee
        #[arg(long)]
        assignee: Option<String>,
        /// Operating system (required by some Bugzilla installations)
        #[arg(long)]
        op_sys: Option<String>,
        /// Hardware platform (required by some Bugzilla installations)
        #[arg(long)]
        rep_platform: Option<String>,
        /// Bug IDs that this bug blocks (comma-separated)
        #[arg(long, value_delimiter = ',')]
        blocks: Vec<u64>,
        /// Bug IDs that this bug depends on (comma-separated)
        #[arg(long, value_delimiter = ',')]
        depends_on: Vec<u64>,
    },
    /// Show bugs related to the authenticated user
    My {
        /// Show bugs I created (instead of assigned to me)
        #[arg(long)]
        created: bool,
        /// Show bugs I'm CC'd on (instead of assigned to me)
        #[arg(long)]
        cc: bool,
        /// Show all bugs related to me (assigned + created + CC'd)
        #[arg(long, conflicts_with_all = ["created", "cc"])]
        all: bool,
        /// Filter by status
        #[arg(long)]
        status: Option<String>,
        /// Max number of results
        #[arg(long, default_value = "50")]
        limit: u32,
        /// Only return these fields (comma-separated)
        #[arg(long)]
        fields: Option<String>,
        /// Exclude these fields (comma-separated)
        #[arg(long)]
        exclude_fields: Option<String>,
    },
    /// Clone an existing bug, optionally overriding fields
    Clone {
        /// Source bug ID or alias
        id: String,
        /// Override summary
        #[arg(long)]
        summary: Option<String>,
        /// Override product
        #[arg(long)]
        product: Option<String>,
        /// Override component
        #[arg(long)]
        component: Option<String>,
        /// Override version
        #[arg(long)]
        version: Option<String>,
        /// Override description
        #[arg(long)]
        description: Option<String>,
        /// Override priority
        #[arg(long)]
        priority: Option<String>,
        /// Override severity
        #[arg(long)]
        severity: Option<String>,
        /// Override assignee
        #[arg(long)]
        assignee: Option<String>,
        /// Override operating system
        #[arg(long)]
        op_sys: Option<String>,
        /// Override hardware platform
        #[arg(long)]
        rep_platform: Option<String>,
        /// Skip adding "Cloned from bug #N" comment
        #[arg(long)]
        no_comment: bool,
        /// Make the new bug depend on the source bug
        #[arg(long)]
        add_depends_on: bool,
        /// Make the new bug block the source bug
        #[arg(long)]
        add_blocks: bool,
        /// Don't copy the CC list from the source bug
        #[arg(long)]
        no_cc: bool,
        /// Don't copy keywords from the source bug
        #[arg(long)]
        no_keywords: bool,
    },
    /// Update an existing bug (supports multiple IDs for batch updates)
    Update {
        /// Bug ID(s)
        #[arg(required = true, num_args = 1..)]
        ids: Vec<u64>,
        /// New status
        #[arg(long)]
        status: Option<String>,
        /// Resolution (when closing)
        #[arg(long)]
        resolution: Option<String>,
        /// Reassign
        #[arg(long)]
        assignee: Option<String>,
        /// Priority
        #[arg(long)]
        priority: Option<String>,
        /// Severity
        #[arg(long)]
        severity: Option<String>,
        /// Summary
        #[arg(long)]
        summary: Option<String>,
        /// Whiteboard
        #[arg(long)]
        whiteboard: Option<String>,
        /// Set flags (e.g. "review?(user@example.com)")
        #[arg(long)]
        flag: Vec<String>,
        /// Add bug IDs to blocks list (comma-separated)
        #[arg(long, value_delimiter = ',')]
        blocks_add: Vec<u64>,
        /// Remove bug IDs from blocks list (comma-separated)
        #[arg(long, value_delimiter = ',')]
        blocks_remove: Vec<u64>,
        /// Add bug IDs to depends-on list (comma-separated)
        #[arg(long, value_delimiter = ',')]
        depends_on_add: Vec<u64>,
        /// Remove bug IDs from depends-on list (comma-separated)
        #[arg(long, value_delimiter = ',')]
        depends_on_remove: Vec<u64>,
    },
}
