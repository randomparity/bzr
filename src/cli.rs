use clap::{Parser, Subcommand};

use crate::config::{ApiMode, AuthMethod};
use crate::types::OutputFormat;
use crate::types::ProductListType;

#[derive(Parser)]
#[command(name = "bzr", version, about = "A CLI for Bugzilla")]
pub struct Cli {
    /// Server name from config (uses default if not set)
    #[arg(long, global = true)]
    pub server: Option<String>,

    /// Output format: table or json
    #[arg(long, global = true)]
    pub output: Option<OutputFormat>,

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
        #[arg(long)]
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
        r#type: ProductListType,
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

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_bug_list_minimal() {
        let cli = Cli::try_parse_from(["bzr", "bug", "list"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Bug {
                action: BugAction::List { .. }
            }
        ));
    }

    #[test]
    fn parse_bug_view_by_id() {
        let cli = Cli::try_parse_from(["bzr", "bug", "view", "12345"]).unwrap();
        match cli.command {
            Commands::Bug {
                action: BugAction::View { id, .. },
            } => assert_eq!(id, "12345"),
            _ => panic!("expected Bug View"),
        }
    }

    #[test]
    fn parse_global_json_flag() {
        let cli = Cli::try_parse_from(["bzr", "--json", "bug", "list"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn parse_global_server_flag() {
        let cli = Cli::try_parse_from(["bzr", "--server", "myserver", "bug", "list"]).unwrap();
        assert_eq!(cli.server.as_deref(), Some("myserver"));
    }

    #[test]
    fn parse_config_set_server() {
        let cli = Cli::try_parse_from([
            "bzr",
            "config",
            "set-server",
            "prod",
            "--url",
            "https://bz.example.com",
            "--api-key",
            "secret123",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Config {
                action: ConfigAction::SetServer { .. }
            }
        ));
    }

    #[test]
    fn parse_unknown_command_fails() {
        let result = Cli::try_parse_from(["bzr", "nonexistent"]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_whoami() {
        let cli = Cli::try_parse_from(["bzr", "whoami"]).unwrap();
        assert!(matches!(cli.command, Commands::Whoami));
    }

    #[test]
    fn parse_bug_search() {
        let cli = Cli::try_parse_from(["bzr", "bug", "search", "crash"]).unwrap();
        match cli.command {
            Commands::Bug {
                action: BugAction::Search { query, limit, .. },
            } => {
                assert_eq!(query, "crash");
                assert_eq!(limit, 50);
            }
            _ => panic!("expected Bug Search"),
        }
    }

    #[test]
    fn parse_bug_search_with_limit() {
        let cli = Cli::try_parse_from(["bzr", "bug", "search", "crash", "--limit", "10"]).unwrap();
        match cli.command {
            Commands::Bug {
                action: BugAction::Search { limit, .. },
            } => assert_eq!(limit, 10),
            _ => panic!("expected Bug Search"),
        }
    }

    #[test]
    fn parse_bug_history() {
        let cli = Cli::try_parse_from(["bzr", "bug", "history", "42"]).unwrap();
        match cli.command {
            Commands::Bug {
                action: BugAction::History { id, since },
            } => {
                assert_eq!(id, 42);
                assert!(since.is_none());
            }
            _ => panic!("expected Bug History"),
        }
    }

    #[test]
    fn parse_bug_create() {
        let cli = Cli::try_parse_from([
            "bzr",
            "bug",
            "create",
            "--product",
            "TestProduct",
            "--component",
            "General",
            "--summary",
            "Test bug",
        ])
        .unwrap();
        match cli.command {
            Commands::Bug {
                action:
                    BugAction::Create {
                        product,
                        component,
                        summary,
                        version,
                        ..
                    },
            } => {
                assert_eq!(product, "TestProduct");
                assert_eq!(component, "General");
                assert_eq!(summary, "Test bug");
                assert_eq!(version, "unspecified");
            }
            _ => panic!("expected Bug Create"),
        }
    }

    #[test]
    fn parse_bug_update_with_flags() {
        let cli = Cli::try_parse_from([
            "bzr", "bug", "update", "42", "--status", "RESOLVED", "--flag", "review+",
        ])
        .unwrap();
        match cli.command {
            Commands::Bug {
                action:
                    BugAction::Update {
                        id, status, flag, ..
                    },
            } => {
                assert_eq!(id, 42);
                assert_eq!(status.as_deref(), Some("RESOLVED"));
                assert_eq!(flag, vec!["review+"]);
            }
            _ => panic!("expected Bug Update"),
        }
    }

    #[test]
    fn parse_comment_list() {
        let cli = Cli::try_parse_from(["bzr", "comment", "list", "99"]).unwrap();
        match cli.command {
            Commands::Comment {
                action: CommentAction::List { bug_id, .. },
            } => assert_eq!(bug_id, 99),
            _ => panic!("expected Comment List"),
        }
    }

    #[test]
    fn parse_comment_add_with_body() {
        let cli =
            Cli::try_parse_from(["bzr", "comment", "add", "42", "--body", "This is a comment"])
                .unwrap();
        match cli.command {
            Commands::Comment {
                action: CommentAction::Add { bug_id, body },
            } => {
                assert_eq!(bug_id, 42);
                assert_eq!(body.as_deref(), Some("This is a comment"));
            }
            _ => panic!("expected Comment Add"),
        }
    }

    #[test]
    fn parse_attachment_list() {
        let cli = Cli::try_parse_from(["bzr", "attachment", "list", "42"]).unwrap();
        match cli.command {
            Commands::Attachment {
                action: AttachmentAction::List { bug_id },
            } => assert_eq!(bug_id, 42),
            _ => panic!("expected Attachment List"),
        }
    }

    #[test]
    fn parse_attachment_download() {
        let cli = Cli::try_parse_from(["bzr", "attachment", "download", "100"]).unwrap();
        match cli.command {
            Commands::Attachment {
                action: AttachmentAction::Download { id, output },
            } => {
                assert_eq!(id, 100);
                assert!(output.is_none());
            }
            _ => panic!("expected Attachment Download"),
        }
    }

    #[test]
    fn parse_product_list() {
        let cli = Cli::try_parse_from(["bzr", "product", "list"]).unwrap();
        match cli.command {
            Commands::Product {
                action: ProductAction::List { r#type },
            } => assert_eq!(r#type, ProductListType::Accessible),
            _ => panic!("expected Product List"),
        }
    }

    #[test]
    fn parse_product_view() {
        let cli = Cli::try_parse_from(["bzr", "product", "view", "Firefox"]).unwrap();
        match cli.command {
            Commands::Product {
                action: ProductAction::View { name },
            } => assert_eq!(name, "Firefox"),
            _ => panic!("expected Product View"),
        }
    }

    #[test]
    fn parse_user_search() {
        let cli = Cli::try_parse_from(["bzr", "user", "search", "alice"]).unwrap();
        match cli.command {
            Commands::User {
                action: UserAction::Search { query, details },
            } => {
                assert_eq!(query, "alice");
                assert!(!details);
            }
            _ => panic!("expected User Search"),
        }
    }

    #[test]
    fn parse_group_add_user() {
        let cli = Cli::try_parse_from([
            "bzr",
            "group",
            "add-user",
            "--group",
            "admin",
            "--user",
            "alice@test.com",
        ])
        .unwrap();
        match cli.command {
            Commands::Group {
                action: GroupAction::AddUser { group, user },
            } => {
                assert_eq!(group, "admin");
                assert_eq!(user, "alice@test.com");
            }
            _ => panic!("expected Group AddUser"),
        }
    }

    #[test]
    fn parse_field_list() {
        let cli = Cli::try_parse_from(["bzr", "field", "list", "status"]).unwrap();
        match cli.command {
            Commands::Field {
                action: FieldAction::List { name },
            } => assert_eq!(name, "status"),
            _ => panic!("expected Field List"),
        }
    }

    #[test]
    fn parse_server_info() {
        let cli = Cli::try_parse_from(["bzr", "server", "info"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Server {
                action: ServerAction::Info
            }
        ));
    }

    #[test]
    fn parse_classification_view() {
        let cli = Cli::try_parse_from(["bzr", "classification", "view", "Unclassified"]).unwrap();
        match cli.command {
            Commands::Classification {
                action: ClassificationAction::View { name },
            } => assert_eq!(name, "Unclassified"),
            _ => panic!("expected Classification View"),
        }
    }

    #[test]
    fn parse_component_create() {
        let cli = Cli::try_parse_from([
            "bzr",
            "component",
            "create",
            "--product",
            "TestProduct",
            "--name",
            "Backend",
            "--description",
            "Backend component",
            "--default-assignee",
            "dev@test.com",
        ])
        .unwrap();
        match cli.command {
            Commands::Component {
                action:
                    ComponentAction::Create {
                        product,
                        name,
                        description,
                        default_assignee,
                    },
            } => {
                assert_eq!(product, "TestProduct");
                assert_eq!(name, "Backend");
                assert_eq!(description, "Backend component");
                assert_eq!(default_assignee, "dev@test.com");
            }
            _ => panic!("expected Component Create"),
        }
    }

    #[test]
    fn parse_verbose_flag() {
        let cli = Cli::try_parse_from(["bzr", "-vvv", "whoami"]).unwrap();
        assert_eq!(cli.verbose, 3);
    }

    #[test]
    fn parse_no_color_flag() {
        let cli = Cli::try_parse_from(["bzr", "--no-color", "whoami"]).unwrap();
        assert!(cli.no_color);
    }

    #[test]
    fn parse_quiet_flag() {
        let cli = Cli::try_parse_from(["bzr", "--quiet", "whoami"]).unwrap();
        assert!(cli.quiet);
    }

    #[test]
    fn parse_api_override() {
        let cli = Cli::try_parse_from(["bzr", "--api", "xmlrpc", "whoami"]).unwrap();
        assert_eq!(cli.api, Some(ApiMode::XmlRpc));
    }

    #[test]
    fn parse_config_set_default() {
        let cli = Cli::try_parse_from(["bzr", "config", "set-default", "prod"]).unwrap();
        match cli.command {
            Commands::Config {
                action: ConfigAction::SetDefault { name },
            } => assert_eq!(name, "prod"),
            _ => panic!("expected Config SetDefault"),
        }
    }

    #[test]
    fn parse_config_show() {
        let cli = Cli::try_parse_from(["bzr", "config", "show"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Config {
                action: ConfigAction::Show
            }
        ));
    }

    #[test]
    fn parse_bug_list_with_filters() {
        let cli = Cli::try_parse_from([
            "bzr",
            "bug",
            "list",
            "--product",
            "Firefox",
            "--status",
            "NEW",
            "--limit",
            "25",
        ])
        .unwrap();
        match cli.command {
            Commands::Bug {
                action:
                    BugAction::List {
                        product,
                        status,
                        limit,
                        ..
                    },
            } => {
                assert_eq!(product.as_deref(), Some("Firefox"));
                assert_eq!(status.as_deref(), Some("NEW"));
                assert_eq!(limit, 25);
            }
            _ => panic!("expected Bug List"),
        }
    }

    #[test]
    fn parse_comment_tag() {
        let cli = Cli::try_parse_from([
            "bzr", "comment", "tag", "200", "--add", "spam", "--remove", "good",
        ])
        .unwrap();
        match cli.command {
            Commands::Comment {
                action:
                    CommentAction::Tag {
                        comment_id,
                        add,
                        remove,
                    },
            } => {
                assert_eq!(comment_id, 200);
                assert_eq!(add, vec!["spam"]);
                assert_eq!(remove, vec!["good"]);
            }
            _ => panic!("expected Comment Tag"),
        }
    }
}
