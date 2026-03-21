mod attachment;
mod bug;
mod classification;
mod comment;
mod component;
mod config;
mod field;
mod group;
mod product;
mod server;
mod template;
mod user;

pub use attachment::AttachmentAction;
pub use bug::BugAction;
pub use classification::ClassificationAction;
pub use comment::CommentAction;
pub use component::ComponentAction;
pub use config::ConfigAction;
pub use field::FieldAction;
pub use group::GroupAction;
pub use product::ProductAction;
pub use server::ServerAction;
pub use template::TemplateAction;
pub use user::UserAction;

use clap::{Parser, Subcommand};

use crate::types::{ApiMode, OutputFormat};

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
    /// Bug template management
    Template {
        #[command(subcommand)]
        action: TemplateAction,
    },
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::types::ProductListType;
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
                assert_eq!(product.as_deref(), Some("TestProduct"));
                assert_eq!(component.as_deref(), Some("General"));
                assert_eq!(summary, "Test bug");
                assert_eq!(version, None);
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
                        ids, status, flag, ..
                    },
            } => {
                assert_eq!(ids, vec![42]);
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
                action: AttachmentAction::Download { id, out },
            } => {
                assert_eq!(id, 100);
                assert!(out.is_none());
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
