mod attachment;
mod bug;
mod classification;
mod comment;
mod component;
mod config;
mod field;
mod fields;
mod group;
mod product;
mod query;
mod server;
mod skills;
mod template;
mod user;

pub(crate) use attachment::{AttachmentAction, UpdateArgs as AttachmentUpdateArgs, UploadArgs};
pub(crate) use bug::{
    AdjacencyArgs, CloneArgs, CloseArgs, CreateArgs, DupArgs, HistoryArgs, LinksArgs, ListArgs,
    MyArgs, ReopenArgs, ResolveArgs, SearchArgs, UpdateArgs, ViewArgs,
};
pub(crate) use bug::{
    BugAction, BugActorFilterArgs, BugFilterArgs, CommentArgs, FieldArgs, PageArgs, SortArgs,
};
// Flattened create-field arg groups are referenced only by in-crate parser
// tests (via `crate::cli::CreateFieldArgs`); gated so the re-export is not
// dead code in non-test builds.
#[cfg(test)]
pub(crate) use bug::{CloneCreateFieldArgs, CreateFieldArgs};
pub(crate) use classification::ClassificationAction;
pub(crate) use comment::CommentAction;
pub(crate) use component::ComponentAction;
pub(crate) use config::ConfigAction;
pub(crate) use field::FieldAction;
pub(crate) use fields::ProjectionArgs;
pub(crate) use group::GroupAction;
pub(crate) use product::ProductAction;
pub(crate) use query::{
    DeleteArgs, QueryAction, RunArgs, SaveArgs, ShowArgs, UpdateArgs as QueryUpdateArgs,
};
// Referenced only by in-crate query parser tests (via
// `crate::cli::QueryRunFilterArgs`); gated to avoid dead-code in non-test builds.
#[cfg(test)]
pub(crate) use query::QueryRunFilterArgs;
pub(crate) use server::ServerAction;
pub(crate) use skills::{AgentTarget, InstallArgs, SkillsAction};
pub(crate) use template::{TemplateAction, TemplateFields, UpdateArgs as TemplateUpdateArgs};
pub(crate) use user::UserAction;

use clap::{Parser, Subcommand};

use crate::types::output::OutputFormat;
use crate::types::transport::ApiMode;

/// A command-line client for Bugzilla servers.
///
/// bzr provides scriptable access to bugs, comments, attachments,
/// products, components, users, and groups across one or more named
/// Bugzilla servers over REST, XML-RPC, or hybrid API transport. It is
/// modeled on the GitHub CLI (`gh`): every
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
///   bzr bug view 12345 --json | jq .data.summary
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
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent global on/off CLI switches (--json, --no-color, --quiet, --dry-run); a state machine does not model unrelated flags"
)]
pub struct Cli {
    /// Server name from config (uses default if not set).
    #[arg(long, global = true)]
    pub server: Option<String>,

    /// Connect to an ad-hoc server by URL, without using config.
    ///
    /// Defines an ephemeral server for this one invocation: nothing is read
    /// from or written to the config file, so a fully stateless run needs no
    /// `config set-server` first. `--server-api-key-env` is optional for
    /// public read-only commands, but required for writes and identity-derived
    /// commands. Conflicts with `--server` (a named server and an inline one
    /// are mutually exclusive). Pairs with `--config` for sandboxed throwaway
    /// runs.
    ///
    ///   bzr --server-url <https://bz.example.com> \
    ///     --server-api-key-env BZR_KEY bug view 123
    #[arg(
        long,
        value_name = "URL",
        global = true,
        conflicts_with = "server",
        verbatim_doc_comment
    )]
    pub server_url: Option<String>,

    /// Environment variable holding the API key for `--server-url`.
    ///
    /// The key is read from this variable, never passed as a literal flag, so
    /// the secret stays out of the process argument list. Only meaningful with
    /// `--server-url`.
    #[arg(long, value_name = "ENV", global = true, requires = "server_url")]
    pub server_api_key_env: Option<String>,

    /// Login email for `--server-url` (Bugzilla 5.0/5.2 whoami fallback).
    ///
    /// Optional. Mirrors the per-server `email` config field; needed only for
    /// the Bugzilla 5.0/5.2 fallback; 5.3+/BMO-derived servers use native
    /// `whoami`. Only meaningful with `--server-url`.
    #[arg(long, value_name = "EMAIL", global = true, requires = "server_url")]
    pub server_email: Option<String>,

    /// Accept invalid TLS certificates for `--server-url`.
    ///
    /// Disables certificate validation for this one ad-hoc invocation only.
    /// Nothing is read from or written to config. Mutually exclusive with
    /// `--server-tls-ca-cert`, `--server-tls-pin-sha256`, and
    /// `--server-tls-pin-now`.
    #[arg(
        long,
        global = true,
        requires = "server_url",
        conflicts_with_all = [
            "server_tls_ca_cert",
            "server_tls_pin_sha256",
            "server_tls_pin_now"
        ],
    )]
    pub server_tls_insecure: bool,

    /// PEM CA certificate file for `--server-url`.
    ///
    /// Adds this CA to trust for one ad-hoc invocation only. The path is not
    /// persisted. Mutually exclusive with `--server-tls-insecure`,
    /// `--server-tls-pin-sha256`, and `--server-tls-pin-now`.
    #[arg(
        long,
        value_name = "PATH",
        global = true,
        requires = "server_url",
        conflicts_with_all = [
            "server_tls_insecure",
            "server_tls_pin_sha256",
            "server_tls_pin_now"
        ],
    )]
    pub server_tls_ca_cert: Option<std::path::PathBuf>,

    /// Pin a certificate fingerprint for `--server-url`.
    ///
    /// Uses the `sha256//<base64>` format accepted by named server config and
    /// applies it to this one ad-hoc invocation only. Mutually exclusive with
    /// `--server-tls-insecure`, `--server-tls-ca-cert`, and
    /// `--server-tls-pin-now`.
    #[arg(
        long,
        value_name = "PIN",
        global = true,
        requires = "server_url",
        conflicts_with_all = [
            "server_tls_insecure",
            "server_tls_ca_cert",
            "server_tls_pin_now"
        ],
    )]
    pub server_tls_pin_sha256: Option<String>,

    /// Pin the current certificate for one `--server-url` invocation.
    ///
    /// Connects once, captures the server certificate fingerprint, and uses
    /// that pin for the remaining API calls in this process. The pin is not
    /// persisted. Mutually exclusive with `--server-tls-insecure`,
    /// `--server-tls-ca-cert`, and `--server-tls-pin-sha256`.
    #[arg(
        long,
        global = true,
        requires = "server_url",
        conflicts_with_all = [
            "server_tls_insecure",
            "server_tls_ca_cert",
            "server_tls_pin_sha256"
        ],
    )]
    pub server_tls_pin_now: bool,

    /// Output format: `table` (default at a TTY), `json`, or `ndjson`.
    ///
    /// `ndjson` emits newline-delimited JSON — one compact value per line for
    /// list results (a single object, e.g. one bug or a result envelope, prints
    /// as one compact line) — ideal for streaming into agents and `jq -c`. When
    /// stdout is piped, the default flips to `json` unless this flag or
    /// `BZR_OUTPUT` overrides it. Precedence:
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
    /// Covers stdout and the stderr diagnostic stream. Each stream is
    /// also desaturated automatically when it is not a terminal.
    /// `NO_COLOR` disables stdout color at any value, and stderr
    /// tracing color only when non-empty. `CLICOLOR=0` and
    /// `CLICOLOR_FORCE=1` apply to stdout alone, and `CLICOLOR_FORCE=1`
    /// has no effect once stdout is redirected. This flag takes
    /// precedence over all of them.
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Suppress all stdout output.
    ///
    /// Redirects stdout to /dev/null; tracing logs are also suppressed.
    /// Stderr is still used for errors and warnings. Useful for scripts
    /// that only care about exit codes.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Override preferred API transport: `rest`, `xmlrpc`, or `hybrid`.
    ///
    /// This chooses the preferred transport for API calls. It does
    /// not guarantee that every operation uses only that transport:
    /// some resource methods use transport-specific exceptions when
    /// one Bugzilla API cannot provide equivalent behavior. `rest`
    /// prefers Bugzilla's REST API. `xmlrpc` prefers XML-RPC.
    /// `hybrid` chooses per operation: bug search/view is generally
    /// REST-first with targeted XML-RPC fallback, while comments and
    /// attachments use XML-RPC first to preserve private-data behavior.
    /// Most users won't need this -- bzr probes on first use and
    /// caches the working transport per server (the auto-detected
    /// default depends on the server's Bugzilla version: `hybrid` for
    /// 5.0.x, `rest` for >= 5.1, `xmlrpc` for older).
    /// Pass `rest` here (or set `api_mode = "rest"` in the server's
    /// config block) to skip the empty-result XML-RPC retry on servers
    /// with slow XML-RPC.
    #[arg(long, global = true)]
    pub api: Option<ApiMode>,

    /// Per-request timeout in seconds (default: 30).
    ///
    /// Overrides the built-in request timeout for slow or distant servers.
    /// Takes precedence over the `BZR_TIMEOUT` environment variable. The
    /// connect timeout (10s) is unaffected. Must be at least 1.
    #[arg(long, value_name = "SECS", global = true, value_parser = clap::value_parser!(u64).range(1..))]
    pub timeout: Option<u64>,

    /// Retry transient failures up to N times (default: 0, disabled).
    ///
    /// Uses exponential backoff that honors a `Retry-After` header. 429 and
    /// connect failures (request provably not processed) are retried for any
    /// operation; 5xx responses and read timeouts are retried only for safe
    /// reads (GET/HEAD), never for writes (create, update, comment) where a
    /// replay could duplicate the effect. Off by default; bounded to at most
    /// 10. Exhausted retries surface the usual network error (exit code 5).
    #[arg(long, value_name = "N", global = true, value_parser = clap::value_parser!(u32).range(0..=10))]
    pub retry: Option<u32>,

    /// Emit structured progress events on stderr for long operations.
    ///
    /// With `--progress ndjson`, paginated fetches (`bug list`/`search
    /// --paginate`, `query run --paginate`) and batch `--from-json`
    /// create/update emit newline-delimited JSON progress events
    /// (`page`/`batch`/`done`, and `error` on failure) to stderr, one per line.
    /// stdout is unaffected. Only `ndjson` is supported. Absent the flag, stderr
    /// stays silent (or carries `-v` logs). Intended for non-verbose runs, since
    /// `-v` log lines interleave on the same stream.
    #[arg(long, value_name = "FORMAT", global = true)]
    pub progress: Option<crate::types::ProgressFormat>,

    /// Preview a supported mutation without writing.
    ///
    /// Resolves and validates the request, then prints the would-be payload
    /// and affected IDs with `"action":"dry-run"` instead of calling the
    /// write API. Exits 0 on a valid request. Supported for bug mutations and
    /// product/user/group `create` and `update`, and component `create`; used
    /// on any other command it exits 7.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Skip confirmation prompts for large batch mutations.
    ///
    /// A `bug update` (or `resolve`/`close`/`reopen`) targeting more than 10
    /// bugs prompts for confirmation at an interactive terminal; `--yes`
    /// bypasses that prompt. Non-interactive runs (piped stdin, agents) never
    /// prompt, so this flag is only needed to override an interactive session.
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,

    /// Set log verbosity (default: warnings only, -v=info, -vv=debug, -vvv=trace; `RUST_LOG` overrides)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Subcommand)]
#[expect(
    clippy::doc_markdown,
    reason = "doc examples are literal shell commands; wrapping URLs in <> or identifiers in backticks would degrade copy-paste UX"
)]
#[expect(
    clippy::large_enum_variant,
    reason = "Bug(BugAction) carries the largest arg struct in the CLI surface; boxing it would \
              ripple through every dispatch match arm for a one-time stack-size difference that \
              matters only while parsing args, not in any hot path"
)]
pub(crate) enum Commands {
    /// Operate on bugs: list, view, search, create, clone, update, history.
    ///
    /// The `bug` group is the most commonly used part of bzr. Public read
    /// paths (`list`, `view`, `search`, `history`) work without any
    /// authentication beyond what the configured server requires. Write paths
    /// (`create`, `clone`, `update`) and identity-derived reads (`my`) require
    /// credentials.
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
    /// "Services"). Disabled installations either return a single
    /// "Unclassified" entry or API error 900 to unprivileged users. For error
    /// 900, table output writes the note to stdout. JSON writes an empty
    /// collection and NDJSON emits no stdout records; both put the note on
    /// stderr. A fetched "Unclassified" row is preserved and accompanied by
    /// the note on stderr.
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

    /// List, view, and create components within a Bugzilla product.
    ///
    /// Creating a component requires Bugzilla admin permissions on the
    /// target product. Components belong to exactly one product. Use
    /// `bzr product view <name>` to list a product's existing components.
    ///
    /// Examples:
    ///
    ///   bzr component create --product MyProduct --name Backend \
    ///     --description "Backend services" \
    ///     --default-assignee dev@example.com
    ///
    /// See bzr-component-create(1) for creation details.
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

    /// Print a published JSON Schema for a command's JSON contract.
    ///
    /// `--json` output shapes and selected `--from-json` input payloads
    /// are documented as checked-in JSON Schema (draft 2020-12) so agents can
    /// validate against a contract instead of branching per command. Run
    /// without a name to list the available schema names; pass one to print
    /// that schema:
    ///
    ///   bzr schema                # list schema names
    ///   bzr schema bug            # the bug object (list elements / view)
    ///   bzr schema bug-create-input  # `bug create --from-json` payload
    ///   bzr schema batch-result   # the batch `bug update` envelope
    ///
    /// Purely local: no config, network, or auth.
    #[command(verbatim_doc_comment)]
    Schema {
        /// Schema name to print; omit to list all available schema names.
        name: Option<String>,
    },

    /// Install bzr's bundled agent skills for a named coding agent.
    ///
    /// The bundled skills teach supported coding agents how to work with bzr.
    /// Choose exactly one destination scope: `--global` installs into your
    /// agent-wide skills directory, while `--project <PATH>` installs beneath
    /// an existing project directory. The command never guesses a scope.
    ///
    /// Examples:
    ///
    ///   bzr skills install --agent codex --global
    ///   bzr skills install --agent all --project .
    ///
    /// This command is local: it makes no network calls and needs no configured
    /// Bugzilla server.
    #[command(verbatim_doc_comment)]
    Skills {
        #[command(subcommand)]
        action: SkillsAction,
    },
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "skills_tests.rs"]
mod skills_tests;
