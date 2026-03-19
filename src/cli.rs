use clap::{Parser, Subcommand};

use crate::config::{ApiMode, AuthMethod};

fn parse_auth_method(s: &str) -> std::result::Result<AuthMethod, String> {
    match s {
        "header" => Ok(AuthMethod::Header),
        "query_param" => Ok(AuthMethod::QueryParam),
        _ => Err(format!(
            "invalid auth method '{s}': expected 'header' or 'query_param'"
        )),
    }
}

#[derive(Parser)]
#[command(name = "bzr", version, about = "A CLI for Bugzilla")]
pub struct Cli {
    /// Server name from config (uses default if not set)
    #[arg(long, global = true)]
    pub server: Option<String>,

    /// Output format: table or json
    #[arg(long, global = true)]
    pub output: Option<String>,

    /// Shorthand for --output json
    #[arg(long, global = true)]
    pub json: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Suppress all stdout output
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Override API transport: rest, xmlrpc, or hybrid
    #[arg(long, global = true)]
    pub api: Option<ApiMode>,

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
    /// Classification operations
    Classification {
        #[command(subcommand)]
        action: ClassificationAction,
    },
    /// Component operations
    Component {
        #[command(subcommand)]
        action: ComponentAction,
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
        /// Override auto-detected auth method (`header` or `query_param`)
        #[arg(long, value_parser = parse_auth_method)]
        auth_method: Option<AuthMethod>,
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
    /// List products
    List {
        /// Product type: accessible (default), selectable, or enterable
        #[arg(long, default_value = "accessible")]
        r#type: String,
    },
    /// View product details (components, versions, milestones)
    View {
        /// Product name
        name: String,
    },
    /// Create a new product
    Create {
        /// Product name
        #[arg(long)]
        name: String,
        /// Product description
        #[arg(long)]
        description: String,
        /// Initial version
        #[arg(long, default_value = "unspecified")]
        version: String,
        /// Whether the product is open for bugs
        #[arg(long, default_value = "true")]
        is_open: bool,
    },
    /// Update an existing product
    Update {
        /// Product name
        name: String,
        /// New description
        #[arg(long)]
        description: Option<String>,
        /// Default milestone
        #[arg(long)]
        default_milestone: Option<String>,
        /// Whether the product is open for bugs
        #[arg(long)]
        is_open: Option<bool>,
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
        /// Show extended details (groups, login status)
        #[arg(long)]
        details: bool,
    },
    /// Create a new user
    Create {
        /// User email
        #[arg(long)]
        email: String,
        /// Full name
        #[arg(long)]
        full_name: Option<String>,
        /// Password (optional, generated by server if omitted)
        #[arg(long)]
        password: Option<String>,
    },
    /// Update a user
    Update {
        /// User ID or login name
        user: String,
        /// New real name
        #[arg(long)]
        real_name: Option<String>,
        /// New email
        #[arg(long)]
        email: Option<String>,
        /// Disable login
        #[arg(long)]
        disable_login: Option<bool>,
        /// Custom login denied message (used with --disable-login)
        #[arg(long)]
        login_denied_text: Option<String>,
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

#[derive(Subcommand)]
pub enum ClassificationAction {
    /// View a classification by name or ID
    View {
        /// Classification name or ID
        name: String,
    },
}

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
