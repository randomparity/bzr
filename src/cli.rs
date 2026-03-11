use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bzr", version, about = "A CLI for Bugzilla")]
pub struct Cli {
    /// Server name from config (uses default if not set)
    #[arg(long, global = true)]
    pub server: Option<String>,

    /// Output format: table or json
    #[arg(long, global = true, default_value = "table")]
    pub output: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Bug operations
    Bug {
        #[command(subcommand)]
        action: BugAction,
    },
    /// Comment operations
    Comment {
        #[command(subcommand)]
        action: CommentAction,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

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
        /// Max number of results
        #[arg(long, default_value = "50")]
        limit: u32,
    },
    /// View a bug by ID
    View {
        /// Bug ID
        id: u64,
    },
    /// Search bugs by text query
    Search {
        /// Search query
        query: String,
        /// Max number of results
        #[arg(long, default_value = "50")]
        limit: u32,
    },
    /// Create a new bug
    Create {
        /// Product name
        #[arg(long)]
        product: String,
        /// Component name
        #[arg(long)]
        component: String,
        /// Bug summary
        #[arg(long)]
        summary: String,
        /// Version
        #[arg(long, default_value = "unspecified")]
        version: String,
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
    },
    /// Update an existing bug
    Update {
        /// Bug ID
        id: u64,
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
    },
}

#[derive(Subcommand)]
pub enum CommentAction {
    /// List comments on a bug
    List {
        /// Bug ID
        bug_id: u64,
    },
    /// Add a comment to a bug
    Add {
        /// Bug ID
        bug_id: u64,
        /// Comment text (opens $EDITOR if not provided)
        #[arg(long)]
        body: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Set up a server
    SetServer {
        /// Server alias name
        name: String,
        /// Server URL
        #[arg(long)]
        url: String,
        /// API key
        #[arg(long)]
        api_key: String,
    },
    /// Set the default server
    SetDefault {
        /// Server alias name
        name: String,
    },
    /// Show current configuration
    Show,
}
