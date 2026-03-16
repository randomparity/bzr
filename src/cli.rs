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

    /// Set log verbosity (default: warnings only, -v=info, -vv=debug, -vvv=trace; `RUST_LOG` overrides)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

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
    /// Attachment operations
    Attachment {
        #[command(subcommand)]
        action: AttachmentAction,
    },
    /// Configuration management
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Product operations
    Product {
        #[command(subcommand)]
        action: ProductAction,
    },
    /// Field/value lookup
    Field {
        #[command(subcommand)]
        action: FieldAction,
    },
    /// User operations
    User {
        #[command(subcommand)]
        action: UserAction,
    },
    /// Group membership management
    Group {
        #[command(subcommand)]
        action: GroupAction,
    },
    /// Show the currently authenticated user
    Whoami,
    /// Server diagnostics
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },
}

#[derive(Subcommand)]
pub enum ServerAction {
    /// Show server version and extensions
    Info,
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
        /// Set flags (e.g. "review?(user@example.com)")
        #[arg(long)]
        flag: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum CommentAction {
    /// List comments on a bug
    List {
        /// Bug ID
        bug_id: u64,
        /// Only show comments created after this date (ISO 8601)
        #[arg(long)]
        since: Option<String>,
    },
    /// Add a comment to a bug
    Add {
        /// Bug ID
        bug_id: u64,
        /// Comment text (opens $EDITOR if not provided)
        #[arg(long)]
        body: Option<String>,
    },
    /// Add or remove tags on a comment
    Tag {
        /// Comment ID
        comment_id: u64,
        /// Tags to add
        #[arg(long)]
        add: Vec<String>,
        /// Tags to remove
        #[arg(long)]
        remove: Vec<String>,
    },
    /// Search comments by tag
    SearchTags {
        /// Tag query
        query: String,
    },
}

#[derive(Subcommand)]
pub enum AttachmentAction {
    /// List attachments on a bug
    List {
        /// Bug ID
        bug_id: u64,
    },
    /// Download an attachment
    Download {
        /// Attachment ID
        id: u64,
        /// Output file path (defaults to original filename)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Upload an attachment to a bug
    Upload {
        /// Bug ID
        bug_id: u64,
        /// File to upload
        file: String,
        /// Attachment summary/description
        #[arg(long)]
        summary: Option<String>,
        /// MIME type (auto-detected if not provided)
        #[arg(long)]
        content_type: Option<String>,
        /// Set flags (e.g. "review?(user@example.com)")
        #[arg(long)]
        flag: Vec<String>,
    },
    /// Update an attachment
    Update {
        /// Attachment ID
        id: u64,
        /// New summary
        #[arg(long)]
        summary: Option<String>,
        /// New file name
        #[arg(long)]
        file_name: Option<String>,
        /// New content type
        #[arg(long)]
        content_type: Option<String>,
        /// Mark as obsolete
        #[arg(long)]
        obsolete: Option<bool>,
        /// Mark as patch
        #[arg(long)]
        is_patch: Option<bool>,
        /// Mark as private
        #[arg(long)]
        is_private: Option<bool>,
        /// Set flags (e.g. "review?(user@example.com)")
        #[arg(long)]
        flag: Vec<String>,
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
        /// Login email (required for older Bugzilla servers)
        #[arg(long)]
        email: Option<String>,
    },
    /// Set the default server
    SetDefault {
        /// Server alias name
        name: String,
    },
    /// Show current configuration
    Show,
}

#[derive(Subcommand)]
pub enum ProductAction {
    /// List accessible products
    List,
    /// View product details (components, versions, milestones)
    View {
        /// Product name
        name: String,
    },
}

#[derive(Subcommand)]
pub enum FieldAction {
    /// List valid values for a bug field
    List {
        /// Field name (e.g. status, priority, severity, resolution)
        name: String,
    },
}

#[derive(Subcommand)]
pub enum UserAction {
    /// Search users by name or email
    Search {
        /// Search query
        query: String,
    },
}

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
    },
}
