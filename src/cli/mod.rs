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

pub use attachment::AttachmentAction;
pub use bug::{BugAction, CommentArgs, CreateFieldArgs, FieldArgs, SortArgs};
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
///   bzr config set-server prod --url <https://bugzilla.example.com> \
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
#[command(
    name = "bzr",
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("BZR_GIT_SHA"), ")"),
    verbatim_doc_comment
)]
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

    /// Use an alternate config file instead of the default.
    ///
    /// Points bzr at a specific `config.toml` for both reads and writes,
    /// sandboxing the invocation from the user's normal config. Takes
    /// precedence over the `BZR_CONFIG` environment variable, which in turn
    /// overrides the default `$XDG_CONFIG_HOME/bzr/config.toml` (or the
    /// platform config directory). Useful for CI, throwaway agent runs, and
    /// per-profile configs.
    #[arg(long, value_name = "PATH", global = true)]
    pub config: Option<std::path::PathBuf>,

    /// Disable colored output.
    ///
    /// bzr also honors the `NO_COLOR` and `CLICOLOR=0` environment
    /// variables, plus `CLICOLOR_FORCE=1` to re-enable. This flag
    /// takes precedence over all of them.
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Suppress all stdout output.
    ///
    /// Redirects stdout to /dev/null; tracing logs are also suppressed.
    /// Stderr is still used for errors and warnings. Useful for scripts
    /// that only care about exit codes.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Override API transport: `rest`, `xmlrpc`, or `hybrid`.
    ///
    /// `rest` uses Bugzilla's REST API exclusively. `xmlrpc` uses
    /// XML-RPC for every call. `hybrid` uses REST where possible
    /// and falls back to XML-RPC for endpoints REST can't express
    /// reliably (e.g. `bzr user create` on Bugzilla 5.3+ when
    /// `use_email_as_login` is disabled). Most users won't need
    /// this -- bzr probes on first use and caches the working
    /// transport per server (the auto-detected default depends on
    /// the server's Bugzilla version: `hybrid` for 5.0.x, `rest`
    /// for >= 5.1, `xmlrpc` for older). Pass `rest` here (or set
    /// `api_mode = "rest"` in the server's config block) to
    /// skip the empty-result XML-RPC retry on servers with
    /// slow XML-RPC.
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
    ///   bzr config set-server prod --url <https://bz.example.com> \
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
    /// resolves to.
    ///
    /// Examples:
    ///
    ///   bzr whoami
    ///   bzr whoami --json
    ///   bzr --server staging whoami
    ///
    /// Exit codes: 0 on success, 9 on auth failure (key invalid or
    /// missing), 13 on TLS pin mismatch.
    #[command(verbatim_doc_comment)]
    Whoami,

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

    /// Generate a shell completion script.
    ///
    /// Emits a completion script for the named shell to stdout. The script
    /// is generated from bzr's live command tree, so it always matches the
    /// installed binary's subcommands and flags. This command is local: it
    /// makes no network calls and needs no configured server.
    ///
    /// Install (pick your shell):
    ///
    ///   # bash (current user)
    ///   bzr completion bash > ~/.local/share/bash-completion/completions/bzr
    ///
    ///   # zsh (ensure the dir is on $fpath, then restart the shell)
    ///   bzr completion zsh > ~/.zfunc/_bzr
    ///
    ///   # fish
    ///   bzr completion fish > ~/.config/fish/completions/bzr.fish
    ///
    ///   # powershell (add to your $PROFILE)
    ///   bzr completion powershell >> $PROFILE
    ///
    /// See bzr-completion(1) for per-shell install detail.
    #[command(verbatim_doc_comment)]
    Completion {
        /// Shell to generate completions for.
        shell: clap_complete::Shell,
    },
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
