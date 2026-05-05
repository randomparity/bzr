mod attachment;
mod bug;
mod classification;
mod comment;
mod component;
mod config;
mod field;
mod group;
mod product;
mod query;
mod server;
mod template;
mod user;
mod whoami;

pub use attachment::AttachmentAction;
pub use bug::BugAction;
pub use classification::ClassificationAction;
pub use comment::CommentAction;
pub use component::ComponentAction;
pub use config::ConfigAction;
pub use field::FieldAction;
pub use group::GroupAction;
pub use product::ProductAction;
pub use query::QueryAction;
pub use server::ServerAction;
pub use template::TemplateAction;
pub use user::UserAction;
pub use whoami::WhoamiAction;

use clap::{Parser, Subcommand};

use crate::types::{ApiMode, OutputFormat};

/// A command-line client for Bugzilla REST API servers.
///
/// bzr provides scriptable access to bugs, comments, attachments,
/// products, components, users, and groups across one or more named
/// Bugzilla servers. It is modeled on the GitHub CLI (`gh`): every
/// resource is a top-level subcommand, every action is a verb under
/// that resource, and `--json` is supported on read paths so output
/// can be consumed by downstream tools.
///
/// Configuration lives in `~/.config/bzr/config.toml` and stores one
/// or more named servers. Use `bzr config set-server` to add a server
/// and `bzr config set-default` to pick the one used when `--server`
/// is omitted. API keys can be stored inline, in an environment
/// variable, or in the OS keychain (`bzr config set-keyring`).
///
/// Output defaults to a colored table at a TTY and to JSON when
/// stdout is piped. Use `--json` or `BZR_OUTPUT=json` to force JSON.
/// Exit codes are stable; the most common are 0 (success), 2 (not
/// found or bad args), 4 (Bugzilla API error), 9 (auth), and 13 (TLS
/// pin mismatch). The full table is in `docs/bzr-cli.md`.
///
/// Examples:
///
///   bzr config set-server prod --url https://bugzilla.example.com \
///     --api-key-env BZR_API_KEY
///   bzr bug list --product Firefox --status NEW --limit 25
///   bzr bug view 12345 --json | jq .summary
///
/// See bzr-bug(1), bzr-config(1), bzr-comment(1), bzr-attachment(1),
/// bzr-product(1), bzr-user(1), bzr-group(1), bzr-template(1),
/// bzr-query(1), bzr-server(1), bzr-classification(1),
/// bzr-component(1), bzr-field(1), and bzr-whoami(1) for the
/// per-resource reference pages.
#[derive(Parser)]
#[command(name = "bzr", version, verbatim_doc_comment)]
#[expect(
    clippy::doc_markdown,
    reason = "doc examples are literal shell commands; wrapping URLs in <> or identifiers in backticks would degrade copy-paste UX"
)]
pub struct Cli {
    /// Server name from config (uses default if not set).
    #[arg(long, global = true)]
    pub server: Option<String>,

    /// Output format: `table` (default at a TTY) or `json`.
    ///
    /// When stdout is piped, the default flips to `json` unless this
    /// flag or `BZR_OUTPUT` overrides it. Precedence:
    /// `--json` > `--output` > `BZR_OUTPUT` > auto-detect.
    #[arg(long, global = true)]
    pub output: Option<OutputFormat>,

    /// Shorthand for `--output json`.
    ///
    /// Equivalent to `--output json` and takes precedence over both
    /// `--output` and the `BZR_OUTPUT` environment variable.
    #[arg(long, global = true)]
    pub json: bool,

    /// Disable colored output.
    ///
    /// bzr also honors the `NO_COLOR` and `CLICOLOR=0` environment
    /// variables, plus `CLICOLOR_FORCE=1` to re-enable. This flag
    /// takes precedence over all of them.
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Suppress all stdout output.
    ///
    /// Redirects stdout to /dev/null. Stderr (errors, warnings,
    /// `tracing` logs) is unaffected. Useful for scripts that only
    /// care about exit codes.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Override API transport: `rest` (default), `xmlrpc`, or `hybrid`.
    ///
    /// `rest` uses Bugzilla's REST API exclusively. `xmlrpc` uses
    /// XML-RPC for every call. `hybrid` uses REST where possible
    /// and falls back to XML-RPC for endpoints REST can't express
    /// reliably (e.g. `bzr user create` on Bugzilla 5.3+ when
    /// `use_email_as_login` is disabled). Most users won't need
    /// this -- bzr probes on first use and caches the working
    /// transport.
    #[arg(long, global = true)]
    pub api: Option<ApiMode>,

    /// Set log verbosity (default: warnings only, -v=info, -vv=debug, -vvv=trace; `RUST_LOG` overrides)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
#[expect(
    clippy::doc_markdown,
    reason = "doc examples are literal shell commands; wrapping URLs in <> or identifiers in backticks would degrade copy-paste UX"
)]
pub enum Commands {
    /// Operate on bugs: list, view, search, create, clone, update, history.
    ///
    /// The `bug` group is the most commonly used part of bzr. Read paths
    /// (`list`, `view`, `search`, `history`, `my`) work without any
    /// authentication beyond what the configured server requires; write
    /// paths (`create`, `clone`, `update`) require credentials and the
    /// caller must have appropriate Bugzilla permissions on the target
    /// product.
    ///
    /// Filter flags (`--product`, `--component`, `--status`, `--assignee`,
    /// `--creator`, `--priority`, `--severity`) on `list` and `my` are
    /// repeatable for OR semantics and accept a `!` prefix for NOT
    /// (e.g. `--status '!CLOSED'`).
    ///
    /// Examples:
    ///
    ///   bzr bug list --product Firefox --status NEW --limit 25
    ///   bzr bug view 12345
    ///   bzr bug create --product P --component C --summary "Crash"
    ///   bzr bug update 100 200 --status RESOLVED --resolution FIXED
    ///
    /// See bzr-bug-list(1), bzr-bug-view(1), bzr-bug-search(1),
    /// bzr-bug-history(1), bzr-bug-my(1), bzr-bug-create(1),
    /// bzr-bug-clone(1), and bzr-bug-update(1) for per-action detail.
    #[command(verbatim_doc_comment)]
    Bug {
        #[command(subcommand)]
        action: BugAction,
    },

    /// List, add, and tag comments on bugs.
    ///
    /// Read paths (`list`, `search-tags`) work with whatever access the
    /// configured server grants to the bug; write paths (`add`, `tag`)
    /// require credentials. Comments are immutable once posted -- tags
    /// can be added or removed, but the body cannot be edited.
    ///
    /// `comment add` reads its body from `--body`, from `$EDITOR` when
    /// neither `--body` nor stdin is provided, or from stdin when piped.
    ///
    /// Examples:
    ///
    ///   bzr comment list 12345
    ///   bzr comment add 12345 --body "Reproduced on RHEL 9.4"
    ///   echo "see #6789" | bzr comment add 12345
    ///
    /// See bzr-comment-list(1), bzr-comment-add(1), bzr-comment-tag(1),
    /// and bzr-comment-search-tags(1) for per-action detail.
    #[command(verbatim_doc_comment)]
    Comment {
        #[command(subcommand)]
        action: CommentAction,
    },

    /// List, download, upload, and update bug attachments.
    ///
    /// Read paths (`list`, `download`) work with whatever access the
    /// configured server grants to the bug; write paths (`upload`,
    /// `update`) require credentials. MIME types on upload are
    /// auto-detected from the file extension and may be overridden with
    /// `--content-type`. Base64 encoding/decoding for the Bugzilla REST
    /// API is handled transparently -- paths in `--out` and the file
    /// argument to `upload` are plain on-disk files.
    ///
    /// Examples:
    ///
    ///   bzr attachment list 12345
    ///   bzr attachment download 9876 --out patch.diff
    ///   bzr attachment upload 12345 patch.diff --summary "Fix crash"
    ///
    /// See bzr-attachment-list(1), bzr-attachment-download(1),
    /// bzr-attachment-upload(1), and bzr-attachment-update(1) for
    /// per-action detail.
    #[command(verbatim_doc_comment)]
    Attachment {
        #[command(subcommand)]
        action: AttachmentAction,
    },

    /// Manage local bzr configuration: servers, default, keychain credentials.
    ///
    /// All `config` actions are local file I/O against
    /// `~/.config/bzr/config.toml` (or `$XDG_CONFIG_HOME/bzr/config.toml`).
    /// They make no network calls and do not require a server to be
    /// reachable. Auth credentials may be stored inline (`--api-key`),
    /// in an environment variable (`--api-key-env`), or in the OS
    /// keychain (`set-keyring`).
    ///
    /// `config show` redacts API keys and prints credential indirection
    /// (env-var name or keychain entry) without reading the secret.
    ///
    /// Examples:
    ///
    ///   bzr config set-server prod --url https://bz.example.com \
    ///     --api-key-env BZR_API_KEY
    ///   bzr config set-default prod
    ///   bzr config set-keyring prod
    ///   bzr config show
    ///
    /// See bzr-config-set-server(1), bzr-config-set-default(1),
    /// bzr-config-show(1), bzr-config-set-keyring(1),
    /// bzr-config-unset-keyring(1), and bzr-config-migrate-to-keyring(1)
    /// for per-action detail.
    #[command(verbatim_doc_comment)]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// List, view, create, and update Bugzilla products.
    ///
    /// Read paths (`list`, `view`) work with whatever access the
    /// configured server grants; write paths (`create`, `update`)
    /// require Bugzilla admin permissions. `list --type` selects the
    /// listing scope: `accessible` (default), `selectable`, or
    /// `enterable`.
    ///
    /// Examples:
    ///
    ///   bzr product list
    ///   bzr product view Firefox
    ///   bzr product create --name MyProduct --description "..."
    ///
    /// See bzr-product-list(1), bzr-product-view(1),
    /// bzr-product-create(1), and bzr-product-update(1) for per-action
    /// detail.
    #[command(verbatim_doc_comment)]
    Product {
        #[command(subcommand)]
        action: ProductAction,
    },

    /// Discover valid values for Bugzilla bug fields (status, priority, etc.).
    ///
    /// Used to find the legal values for fields like `--status`,
    /// `--priority`, `--severity`, `--resolution`, and `--op-sys` before
    /// passing them to `bug create` or `bug update`. Field-name aliases
    /// (e.g. `severity` for `bug_severity`) are listed by `field aliases`.
    ///
    /// Examples:
    ///
    ///   bzr field aliases
    ///   bzr field list status
    ///   bzr field list priority --json
    ///
    /// See bzr-field-aliases(1) and bzr-field-list(1) for per-action
    /// detail.
    #[command(verbatim_doc_comment)]
    Field {
        #[command(subcommand)]
        action: FieldAction,
    },

    /// Search, create, and update Bugzilla user accounts.
    ///
    /// `user search` works with whatever access the configured server
    /// grants (typically all authenticated users); `user create` and
    /// `user update` require Bugzilla admin permissions. Some Bugzilla
    /// installations disable `use_email_as_login` -- on those servers,
    /// `--login` is required separately from `--email`.
    ///
    /// Examples:
    ///
    ///   bzr user search alice
    ///   bzr user create --email alice@example.com --name "Alice" \
    ///     --password '...'
    ///
    /// See bzr-user-search(1), bzr-user-create(1), and bzr-user-update(1)
    /// for per-action detail.
    #[command(verbatim_doc_comment)]
    User {
        #[command(subcommand)]
        action: UserAction,
    },

    /// Manage Bugzilla group membership and group definitions.
    ///
    /// All actions in this group require Bugzilla admin permissions.
    /// Group membership changes are immediate; viewing a group lists
    /// its members and metadata. On Bugzilla 5.3+, membership operations
    /// require POST (handled automatically by bzr).
    ///
    /// Examples:
    ///
    ///   bzr group list-users --group editbugs
    ///   bzr group add-user --group editbugs --user alice@example.com
    ///   bzr group view editbugs
    ///
    /// See bzr-group-add-user(1), bzr-group-remove-user(1),
    /// bzr-group-list-users(1), bzr-group-view(1), bzr-group-create(1),
    /// and bzr-group-update(1) for per-action detail.
    #[command(verbatim_doc_comment)]
    Group {
        #[command(subcommand)]
        action: GroupAction,
    },

    /// Show the currently authenticated user.
    ///
    /// Useful as a quick auth smoke test: prints the login name,
    /// real name, and email of the user the configured API key
    /// resolves to. Both `bzr whoami` and `bzr whoami show` are
    /// equivalent -- `show` is the only action and may be omitted.
    ///
    /// Examples:
    ///
    ///   bzr whoami
    ///   bzr whoami show --json
    ///   bzr --server staging whoami
    ///
    /// See bzr-whoami-show(1) for action detail.
    #[command(verbatim_doc_comment)]
    Whoami {
        #[command(subcommand)]
        action: Option<WhoamiAction>,
    },

    /// Inspect the configured Bugzilla server: version, extensions, capabilities.
    ///
    /// Useful for confirming connectivity and detecting feature support
    /// before invoking commands that depend on a specific server version
    /// or extension. The output includes the Bugzilla version string,
    /// a list of installed extensions, and the active API transport
    /// (REST, XML-RPC, or hybrid).
    ///
    /// Examples:
    ///
    ///   bzr server info
    ///   bzr --server staging server info --json
    ///
    /// See bzr-server-info(1) for action detail.
    #[command(verbatim_doc_comment)]
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },

    /// View Bugzilla classifications (the grouping above products).
    ///
    /// Classifications are an optional Bugzilla feature that groups
    /// related products under a shared umbrella (e.g. "Components",
    /// "Services"). Visibility depends on server configuration --
    /// installations with classifications disabled return a single
    /// "Unclassified" entry.
    ///
    /// Examples:
    ///
    ///   bzr classification view Unclassified
    ///   bzr classification view "Red Hat" --json
    ///
    /// See bzr-classification-view(1) for action detail.
    #[command(verbatim_doc_comment)]
    Classification {
        #[command(subcommand)]
        action: ClassificationAction,
    },

    /// Create and update components within a Bugzilla product.
    ///
    /// Both actions require Bugzilla admin permissions on the target
    /// product. Components belong to exactly one product; renaming or
    /// reassigning across products is not supported by the Bugzilla
    /// REST API. Use `bzr product view <name>` to list a product's
    /// existing components.
    ///
    /// Examples:
    ///
    ///   bzr component create --product MyProduct --name Backend \
    ///     --description "Backend services" \
    ///     --default-assignee dev@example.com
    ///   bzr component update --product MyProduct --name Backend \
    ///     --description "Updated description"
    ///
    /// See bzr-component-create(1) and bzr-component-update(1) for
    /// per-action detail.
    #[command(verbatim_doc_comment)]
    Component {
        #[command(subcommand)]
        action: ComponentAction,
    },

    /// Manage local bug-creation templates (saved field defaults).
    ///
    /// Templates store reusable bug-field defaults (product, component,
    /// version, priority, severity, assignee, op-sys, rep-platform,
    /// description) under a name. They are local to bzr -- saved in
    /// `~/.config/bzr/config.toml` -- and never sent to the server.
    /// Apply a template at bug-creation time with `bzr bug create
    /// --template <name>`; CLI flags override template values.
    ///
    /// Contrast with `bzr query` (saved searches), which also stores
    /// data locally but executes against the server when run.
    ///
    /// Examples:
    ///
    ///   bzr template save security-bug --product Security \
    ///     --component Vulnerabilities --severity critical
    ///   bzr template list
    ///   bzr bug create --template security-bug --summary "XSS in login"
    ///
    /// See bzr-template-save(1), bzr-template-list(1),
    /// bzr-template-show(1), and bzr-template-delete(1) for per-action
    /// detail.
    #[command(verbatim_doc_comment)]
    Template {
        #[command(subcommand)]
        action: TemplateAction,
    },

    /// Save and run reusable Bugzilla searches.
    ///
    /// Saved queries pair a set of filter flags (or a parsed
    /// `buglist.cgi` URL) with a name, stored locally in
    /// `~/.config/bzr/config.toml`. `query save` and `query list`/`show`/
    /// `delete` are local-only; `query run` executes the saved filters
    /// against a Bugzilla server (the saved server, or `--server` to
    /// override). Filter flags use the same syntax as `bzr bug list`,
    /// including repeatability for OR and `!` prefix for NOT.
    ///
    /// `query save --from-url` parses a Bugzilla `buglist.cgi` URL and
    /// extracts known filter parameters; unrecognized parameters are
    /// preserved verbatim and passed through to the API.
    ///
    /// Examples:
    ///
    ///   bzr query save firefox-new --product Firefox --status NEW
    ///   bzr query save my-saved --from-url 'https://bz/buglist.cgi?...'
    ///   bzr query run firefox-new --limit 10
    ///   bzr query run firefox-new --server staging
    ///
    /// See bzr-query-save(1), bzr-query-list(1), bzr-query-show(1),
    /// bzr-query-delete(1), and bzr-query-run(1) for per-action detail.
    #[command(verbatim_doc_comment)]
    Query {
        #[command(subcommand)]
        action: QueryAction,
    },
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::types::ProductListType;
    use clap::{CommandFactory, Parser};

    /// Doc-comment coverage for items already converted to multi-paragraph
    /// `long_about` per docs/dev/cli-doc-style.md. Phase 1 + 2a of the
    /// CLI doc-expansion plan; extend as phases 2b/2c land. When this list
    /// covers every leaf subcommand, drop the explicit list and walk the
    /// command tree instead.
    const COVERED_PATHS: &[&[&str]] = &[
        // Phase 1: top-level + 14 group variants
        &[],
        &["bug"],
        &["comment"],
        &["attachment"],
        &["config"],
        &["product"],
        &["field"],
        &["user"],
        &["group"],
        &["whoami"],
        &["server"],
        &["classification"],
        &["component"],
        &["template"],
        &["query"],
        // Phase 2a: bug.rs actions
        &["bug", "list"],
        &["bug", "view"],
        &["bug", "search"],
        &["bug", "history"],
        &["bug", "create"],
        &["bug", "my"],
        &["bug", "clone"],
        &["bug", "update"],
        // Phase 2a: config.rs actions
        &["config", "set-server"],
        &["config", "set-default"],
        &["config", "show"],
        &["config", "set-keyring"],
        &["config", "unset-keyring"],
        &["config", "migrate-to-keyring"],
        // Phase 2a: query.rs actions
        &["query", "save"],
        &["query", "list"],
        &["query", "show"],
        &["query", "delete"],
        &["query", "run"],
        // Phase 2b: attachment.rs actions
        &["attachment", "list"],
        &["attachment", "download"],
        &["attachment", "upload"],
        &["attachment", "update"],
        // Phase 2b: comment.rs actions
        &["comment", "list"],
        &["comment", "add"],
        &["comment", "tag"],
        &["comment", "search-tags"],
        // Phase 2b: user.rs actions
        &["user", "search"],
        &["user", "create"],
        &["user", "update"],
        // Phase 2b: group.rs actions
        &["group", "add-user"],
        &["group", "remove-user"],
        &["group", "list-users"],
        &["group", "view"],
        &["group", "create"],
        &["group", "update"],
        // Phase 2b: product.rs actions
        &["product", "list"],
        &["product", "view"],
        &["product", "create"],
        &["product", "update"],
        // Phase 2b: template.rs actions
        &["template", "save"],
        &["template", "list"],
        &["template", "show"],
        &["template", "delete"],
        // Phase 2b: component.rs actions
        &["component", "create"],
        &["component", "update"],
        // Phase 2c: trivial files
        &["whoami", "show"],
        &["server", "info"],
        &["classification", "view"],
        &["field", "aliases"],
        &["field", "list"],
    ];

    #[test]
    fn cli_doc_long_about_coverage() {
        let cmd = Cli::command();
        for path in COVERED_PATHS {
            let mut current = &cmd;
            for &name in *path {
                let next = current.get_subcommands().find(|c| c.get_name() == name);
                current = next
                    .unwrap_or_else(|| panic!("subcommand path {path:?} not found in clap tree"));
            }
            let about = current.get_about().map(ToString::to_string);
            let long_about = current.get_long_about().map(ToString::to_string);
            assert!(
                long_about.is_some(),
                "subcommand path {path:?} is missing long_about (multi-paragraph doc comment required)"
            );
            assert_ne!(
                about, long_about,
                "subcommand path {path:?} has long_about identical to about (single-paragraph doc -- expand to multi-paragraph per docs/dev/cli-doc-style.md)"
            );
        }
    }

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
    fn parse_config_set_server_with_env_var() {
        let cli = Cli::try_parse_from([
            "bzr",
            "config",
            "set-server",
            "prod",
            "--url",
            "https://bz.example.com",
            "--api-key-env",
            "BZR_API_KEY",
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
        let cli = Cli::try_parse_from(["bzr", "whoami", "show"]).unwrap();
        assert!(matches!(cli.command, Commands::Whoami { action: _ }));
    }

    #[test]
    fn parse_bug_search() {
        let cli = Cli::try_parse_from(["bzr", "bug", "search", "crash"]).unwrap();
        match cli.command {
            Commands::Bug {
                action: BugAction::Search { query, limit, .. },
            } => {
                assert_eq!(query.as_deref(), Some("crash"));
                assert_eq!(limit, None);
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
            } => assert_eq!(limit, Some(10)),
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
                action:
                    CommentAction::Add {
                        bug_id,
                        body,
                        private,
                    },
            } => {
                assert_eq!(bug_id, 42);
                assert_eq!(body.as_deref(), Some("This is a comment"));
                assert!(!private);
            }
            _ => panic!("expected Comment Add"),
        }
    }

    #[test]
    fn parse_comment_add_with_private() {
        let cli = Cli::try_parse_from([
            "bzr",
            "comment",
            "add",
            "42",
            "--body",
            "secret note",
            "--private",
        ])
        .unwrap();
        match cli.command {
            Commands::Comment {
                action:
                    CommentAction::Add {
                        bug_id,
                        body,
                        private,
                    },
            } => {
                assert_eq!(bug_id, 42);
                assert_eq!(body.as_deref(), Some("secret note"));
                assert!(private);
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
        let cli = Cli::try_parse_from(["bzr", "-vvv", "whoami", "show"]).unwrap();
        assert_eq!(cli.verbose, 3);
    }

    #[test]
    fn parse_no_color_flag() {
        let cli = Cli::try_parse_from(["bzr", "--no-color", "whoami", "show"]).unwrap();
        assert!(cli.no_color);
    }

    #[test]
    fn parse_quiet_flag() {
        let cli = Cli::try_parse_from(["bzr", "--quiet", "whoami", "show"]).unwrap();
        assert!(cli.quiet);
    }

    #[test]
    fn parse_api_override() {
        let cli = Cli::try_parse_from(["bzr", "--api", "xmlrpc", "whoami", "show"]).unwrap();
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
                assert_eq!(product, vec!["Firefox"]);
                assert_eq!(status, vec!["NEW"]);
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

    #[test]
    fn parse_bug_my_defaults() {
        let cli = Cli::try_parse_from(["bzr", "bug", "my"]).unwrap();
        match cli.command {
            Commands::Bug {
                action:
                    BugAction::My {
                        created,
                        cc,
                        all,
                        limit,
                        ..
                    },
            } => {
                assert!(!created);
                assert!(!cc);
                assert!(!all);
                assert_eq!(limit, 50);
            }
            _ => panic!("expected Bug My"),
        }
    }

    #[test]
    fn parse_bug_my_all_conflicts_with_created() {
        let result = Cli::try_parse_from(["bzr", "bug", "my", "--all", "--created"]);
        assert!(result.is_err(), "--all should conflict with --created");
    }

    #[test]
    fn parse_bug_my_all_conflicts_with_cc() {
        let result = Cli::try_parse_from(["bzr", "bug", "my", "--all", "--cc"]);
        assert!(result.is_err(), "--all should conflict with --cc");
    }

    #[test]
    fn parse_bug_clone_minimal() {
        let cli = Cli::try_parse_from(["bzr", "bug", "clone", "123"]).unwrap();
        match cli.command {
            Commands::Bug {
                action: BugAction::Clone { id, summary, .. },
            } => {
                assert_eq!(id, "123");
                assert!(summary.is_none());
            }
            _ => panic!("expected Bug Clone"),
        }
    }

    #[test]
    fn parse_template_save_with_fields() {
        let cli = Cli::try_parse_from([
            "bzr",
            "template",
            "save",
            "security-bug",
            "--product",
            "Security",
            "--component",
            "Vulnerabilities",
            "--severity",
            "critical",
        ])
        .unwrap();
        match cli.command {
            Commands::Template {
                action:
                    TemplateAction::Save {
                        name,
                        product,
                        component,
                        severity,
                        ..
                    },
            } => {
                assert_eq!(name, "security-bug");
                assert_eq!(product.as_deref(), Some("Security"));
                assert_eq!(component.as_deref(), Some("Vulnerabilities"));
                assert_eq!(severity.as_deref(), Some("critical"));
            }
            _ => panic!("expected Template Save"),
        }
    }

    #[test]
    fn parse_query_save_list_kind() {
        let cli = Cli::try_parse_from([
            "bzr",
            "query",
            "save",
            "firefox-new",
            "--product",
            "Firefox",
            "--status",
            "NEW",
            "--limit",
            "25",
        ])
        .unwrap();
        match cli.command {
            Commands::Query {
                action:
                    QueryAction::Save {
                        name,
                        product,
                        status,
                        limit,
                        ..
                    },
            } => {
                assert_eq!(name, "firefox-new");
                assert_eq!(product, vec!["Firefox"]);
                assert_eq!(status, vec!["NEW"]);
                assert_eq!(limit, Some(25));
            }
            _ => panic!("expected Query Save"),
        }
    }

    #[test]
    fn parse_query_save_search_kind() {
        let cli = Cli::try_parse_from([
            "bzr",
            "query",
            "save",
            "crashes",
            "--search",
            "crash in tab",
            "--limit",
            "10",
        ])
        .unwrap();
        match cli.command {
            Commands::Query {
                action:
                    QueryAction::Save {
                        name,
                        search,
                        limit,
                        ..
                    },
            } => {
                assert_eq!(name, "crashes");
                assert_eq!(search.as_deref(), Some("crash in tab"));
                assert_eq!(limit, Some(10));
            }
            _ => panic!("expected Query Save"),
        }
    }

    #[test]
    fn parse_query_run() {
        let cli = Cli::try_parse_from(["bzr", "query", "run", "firefox-new"]).unwrap();
        match cli.command {
            Commands::Query {
                action: QueryAction::Run { name, limit, .. },
            } => {
                assert_eq!(name, "firefox-new");
                assert!(limit.is_none());
            }
            _ => panic!("expected Query Run"),
        }
    }

    #[test]
    fn parse_query_run_with_limit_override() {
        let cli =
            Cli::try_parse_from(["bzr", "query", "run", "firefox-new", "--limit", "10"]).unwrap();
        match cli.command {
            Commands::Query {
                action: QueryAction::Run { name, limit, .. },
            } => {
                assert_eq!(name, "firefox-new");
                assert_eq!(limit, Some(10));
            }
            _ => panic!("expected Query Run"),
        }
    }

    #[test]
    fn parse_query_list() {
        let cli = Cli::try_parse_from(["bzr", "query", "list"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Query {
                action: QueryAction::List
            }
        ));
    }

    #[test]
    fn parse_query_show() {
        let cli = Cli::try_parse_from(["bzr", "query", "show", "firefox-new"]).unwrap();
        match cli.command {
            Commands::Query {
                action: QueryAction::Show { name },
            } => {
                assert_eq!(name, "firefox-new");
            }
            _ => panic!("expected Query Show"),
        }
    }

    #[test]
    fn parse_query_delete() {
        let cli = Cli::try_parse_from(["bzr", "query", "delete", "firefox-new"]).unwrap();
        match cli.command {
            Commands::Query {
                action: QueryAction::Delete { name },
            } => {
                assert_eq!(name, "firefox-new");
            }
            _ => panic!("expected Query Delete"),
        }
    }

    #[test]
    fn parse_set_server_tls_ca_cert() {
        let cli = Cli::try_parse_from([
            "bzr",
            "config",
            "set-server",
            "test",
            "--url",
            "https://example.com",
            "--api-key",
            "key",
            "--tls-ca-cert",
            "/path/to/ca.pem",
        ])
        .unwrap();
        match cli.command {
            Commands::Config {
                action: ConfigAction::SetServer { tls_ca_cert, .. },
            } => assert_eq!(tls_ca_cert.as_deref(), Some("/path/to/ca.pem")),
            _ => panic!("expected Config SetServer"),
        }
    }

    #[test]
    fn parse_set_server_tls_insecure_conflicts_with_ca_cert() {
        let result = Cli::try_parse_from([
            "bzr",
            "config",
            "set-server",
            "test",
            "--url",
            "https://example.com",
            "--api-key",
            "key",
            "--tls-insecure",
            "--tls-ca-cert",
            "/path/to/ca.pem",
        ]);
        assert!(result.is_err(), "should conflict");
    }

    #[test]
    fn parse_set_server_tls_pin_now() {
        let cli = Cli::try_parse_from([
            "bzr",
            "config",
            "set-server",
            "test",
            "--url",
            "https://example.com",
            "--api-key",
            "key",
            "--tls-pin-now",
        ])
        .unwrap();
        match cli.command {
            Commands::Config {
                action: ConfigAction::SetServer { tls_pin_now, .. },
            } => assert!(tls_pin_now),
            _ => panic!("expected Config SetServer"),
        }
    }

    #[test]
    fn parse_set_server_tls_pin_sha256() {
        let cli = Cli::try_parse_from([
            "bzr",
            "config",
            "set-server",
            "test",
            "--url",
            "https://example.com",
            "--api-key",
            "key",
            "--tls-pin-sha256",
            "sha256//abc123",
        ])
        .unwrap();
        match cli.command {
            Commands::Config {
                action: ConfigAction::SetServer { tls_pin_sha256, .. },
            } => assert_eq!(tls_pin_sha256.as_deref(), Some("sha256//abc123")),
            _ => panic!("expected Config SetServer"),
        }
    }

    #[test]
    fn parse_set_server_tls_pin_clear() {
        let cli = Cli::try_parse_from([
            "bzr",
            "config",
            "set-server",
            "test",
            "--url",
            "https://example.com",
            "--api-key",
            "key",
            "--tls-pin-clear",
        ])
        .unwrap();
        match cli.command {
            Commands::Config {
                action: ConfigAction::SetServer { tls_pin_clear, .. },
            } => assert!(tls_pin_clear),
            _ => panic!("expected Config SetServer"),
        }
    }

    #[test]
    fn parse_set_server_tls_pin_sha256_conflicts_with_tls_insecure() {
        let result = Cli::try_parse_from([
            "bzr",
            "config",
            "set-server",
            "test",
            "--url",
            "https://example.com",
            "--api-key",
            "key",
            "--tls-insecure",
            "--tls-pin-sha256",
            "sha256//abc123",
        ]);
        assert!(
            result.is_err(),
            "--tls-insecure should conflict with --tls-pin-sha256"
        );
    }

    #[test]
    fn parse_set_server_tls_pin_now_conflicts_with_tls_pin_sha256() {
        let result = Cli::try_parse_from([
            "bzr",
            "config",
            "set-server",
            "test",
            "--url",
            "https://example.com",
            "--api-key",
            "key",
            "--tls-pin-now",
            "--tls-pin-sha256",
            "sha256//abc123",
        ]);
        assert!(
            result.is_err(),
            "--tls-pin-now should conflict with --tls-pin-sha256"
        );
    }

    #[test]
    fn parse_set_server_tls_pin_clear_conflicts_with_tls_pin_now() {
        let result = Cli::try_parse_from([
            "bzr",
            "config",
            "set-server",
            "test",
            "--url",
            "https://example.com",
            "--api-key",
            "key",
            "--tls-pin-clear",
            "--tls-pin-now",
        ]);
        assert!(
            result.is_err(),
            "--tls-pin-clear should conflict with --tls-pin-now"
        );
    }

    #[test]
    fn parse_whoami_without_subcommand() {
        let cli = Cli::try_parse_from(["bzr", "whoami"]).unwrap();
        match cli.command {
            Commands::Whoami { action } => assert!(action.is_none()),
            _ => panic!("expected Whoami"),
        }
    }

    #[test]
    fn parse_attachment_upload_with_summary() {
        let cli = Cli::try_parse_from([
            "bzr",
            "attachment",
            "upload",
            "42",
            "patch.diff",
            "--summary",
            "Fix crash",
        ])
        .unwrap();
        match cli.command {
            Commands::Attachment {
                action:
                    AttachmentAction::Upload {
                        bug_id,
                        file,
                        summary,
                        ..
                    },
            } => {
                assert_eq!(bug_id, 42);
                assert_eq!(file, "patch.diff");
                assert_eq!(summary.as_deref(), Some("Fix crash"));
            }
            _ => panic!("expected Attachment Upload"),
        }
    }

    #[test]
    fn parse_template_delete() {
        let cli = Cli::try_parse_from(["bzr", "template", "delete", "security-bug"]).unwrap();
        match cli.command {
            Commands::Template {
                action: TemplateAction::Delete { name },
            } => assert_eq!(name, "security-bug"),
            _ => panic!("expected Template Delete"),
        }
    }
}
