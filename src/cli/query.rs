use clap::Subcommand;

/// Argument IDs of the structured filter flags on `query save`. Centralized so
/// the mutual-exclusivity lists on `--from-url` / `--search` stay in sync when a
/// filter flag is added or renamed (rather than hand-maintaining two literal
/// lists). Keep in lockstep with the `Vec<String>` filter fields on `Save`.
const FILTER_FLAG_ARGS: [&str; 15] = [
    "product",
    "component",
    "status",
    "assignee",
    "creator",
    "priority",
    "severity",
    "whiteboard",
    "target_milestone",
    "version",
    "op_sys",
    "platform",
    "resolution",
    "qa_contact",
    "url",
];

#[derive(Subcommand)]
#[expect(
    clippy::doc_markdown,
    reason = "doc examples are literal shell commands; wrapping URLs in <> or identifiers in backticks would degrade copy-paste UX"
)]
pub enum QueryAction {
    /// Save a named query (filter flags or a Bugzilla URL) for later reuse.
    ///
    /// Three mutually exclusive input modes are supported. Filter
    /// flags (`--product`, `--component`, `--status`, ...) store a
    /// structured filter with the same semantics as `bzr bug list`.
    /// `--search "..."` stores a free-text search. `--from-url '...'`
    /// parses a Bugzilla `buglist.cgi` URL into known filters and
    /// preserves unrecognized parameters verbatim.
    ///
    /// `--limit`, `--fields`, and `--exclude-fields` are stored
    /// alongside the query and become run-time defaults; they can be
    /// overridden per invocation by the matching flags on
    /// `bzr query run`.
    ///
    /// `--created-since` / `--changed-since` save Bugzilla
    /// `creation_time` / `last_change_time` filters into the
    /// query. Same accepted forms as `bzr bug list --created-since`.
    /// Validated at save time; malformed input exits 7.
    ///
    /// Eight additional bzl-parity field filters are accepted with the
    /// same semantics as `bzr bug list --whiteboard` etc. (see
    /// bzr-bug-list(1) for syntax and substring vs exact match
    /// distinction).
    ///
    /// Examples:
    ///
    ///   bzr query save firefox-new --product Firefox --status NEW
    ///   bzr query save crashes --search "crash in tab"
    ///   bzr query save my-saved \
    ///     --from-url 'https://bz/buglist.cgi?product=Firefox'
    ///   bzr query save recent-firefox --product Firefox --changed-since 2026-04-01
    ///
    /// See bzr-query-run(1) to execute a saved query,
    /// bzr-query-list(1) for the inventory, and bzr-bug-list(1) for
    /// one-shot listing.
    #[command(verbatim_doc_comment)]
    Save {
        /// Query name
        name: String,
        /// Import query from a Bugzilla `buglist.cgi` URL.
        ///
        /// Parses the URL's query parameters into known filters
        /// where possible; unrecognized parameters are stored
        /// verbatim and passed through at run time. Mutually
        /// exclusive with `--search` and every filter flag.
        #[arg(long, conflicts_with = "search", conflicts_with_all = FILTER_FLAG_ARGS)]
        from_url: Option<String>,
        /// Free-text search query (creates a `search`-kind saved query).
        ///
        /// Mutually exclusive with `--from-url` and the structured
        /// filter flags. Stores the query as a free-text search;
        /// `bzr query run <name>` then issues the same search
        /// against the configured server.
        #[arg(long, conflicts_with_all = FILTER_FLAG_ARGS)]
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
        /// Filter to bugs created at or after this date (saved into the query).
        ///
        /// Accepts the same forms as `bzr bug list --created-since`.
        #[arg(long, value_name = "DATE")]
        created_since: Option<String>,
        /// Filter to bugs last modified at or after this date (saved into the query).
        ///
        /// Accepts the same forms as `bzr bug list --changed-since`.
        #[arg(long, value_name = "DATE")]
        changed_since: Option<String>,
        /// Filter by Status Whiteboard substring (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        whiteboard: Vec<String>,
        /// Filter by Target Milestone (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        target_milestone: Vec<String>,
        /// Filter by Version (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        version: Vec<String>,
        /// Filter by Operating System (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        op_sys: Vec<String>,
        /// Filter by Platform (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        platform: Vec<String>,
        /// Filter by Resolution (repeatable for OR; prefix with ! to exclude); empty matches open bugs
        #[arg(long)]
        resolution: Vec<String>,
        /// Filter by QA Contact login (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        qa_contact: Vec<String>,
        /// Filter by URL field substring (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        url: Vec<String>,
    },
    /// List all saved queries.
    ///
    /// Prints each query's name, kind (filter / search / url), and a
    /// short summary of its parameters. Use `--json` for a structured
    /// listing.
    ///
    /// Examples:
    ///
    ///   bzr query list
    ///   bzr query list --json | jq '.queries[].name'
    ///
    /// See bzr-query-show(1) for the full parameters of one query.
    #[command(verbatim_doc_comment)]
    List,

    /// Show the parameters of one saved query.
    ///
    /// Prints the query's name, kind, every stored parameter, and
    /// any raw URL pass-through values. Useful for verifying what
    /// `bzr query run <name>` will execute.
    ///
    /// Examples:
    ///
    ///   bzr query show firefox-new
    ///   bzr query show firefox-new --json
    ///
    /// See bzr-query-run(1) to execute and bzr-query-list(1) for
    /// the inventory.
    #[command(verbatim_doc_comment)]
    Show {
        /// Query name
        name: String,
    },

    /// Delete a saved query.
    ///
    /// Removes the named query from the local config. Does not
    /// prompt for confirmation -- if you need recovery, restore the
    /// previous `~/.config/bzr/config.toml` from backup or re-create
    /// the query.
    ///
    /// Examples:
    ///
    ///   bzr query delete firefox-new
    ///
    /// See bzr-query-list(1) to verify the query exists first.
    #[command(verbatim_doc_comment)]
    Delete {
        /// Query name
        name: String,
    },

    /// Run a saved query against a Bugzilla server.
    ///
    /// Reads the query's stored parameters (filter flags, free-text
    /// search, or parsed URL) and executes them against the
    /// configured server. Per-invocation overrides: `--limit`
    /// replaces the saved limit, `--fields` / `--exclude-fields`
    /// replace the saved field selections, and `--server` runs the
    /// query against a different server than the one current when
    /// the query was saved.
    ///
    /// The output format matches `bzr bug list` (table by default,
    /// JSON with `--json`).
    ///
    /// `--created-since` / `--changed-since` override the saved
    /// query's date filters for this run; passing them clears
    /// nothing (`None` keeps the saved value), matching the
    /// existing `--limit` / `--fields` override convention.
    ///
    /// All eight bzl-parity field filters from `bzr bug list` are
    /// also overrideable here. Passing a flag replaces the saved
    /// list for that field; omitting it keeps the saved value.
    /// There is no clear sentinel — to clear a saved field, edit
    /// the config or re-save the query.
    ///
    /// Examples:
    ///
    ///   bzr query run firefox-new
    ///   bzr query run firefox-new --limit 10
    ///   bzr query run firefox-new --server staging
    ///   bzr query run recent-firefox --changed-since 2026-05-01
    ///
    /// See bzr-query-save(1) to define a query, bzr-query-show(1)
    /// to inspect what will run, and bzr-bug-list(1) for ad-hoc
    /// listing without a saved query.
    #[command(verbatim_doc_comment)]
    Run {
        /// Query name
        name: String,
        /// Override the saved limit
        #[arg(long)]
        limit: Option<u32>,
        /// Fields to request from the server (comma-separated). Table:
        /// selects which columns to show (in order). --json: the JSON object
        /// contains only the selected fields (id is included only if requested).
        #[arg(long)]
        fields: Option<String>,
        /// Fields to drop from the server request (comma-separated). Table:
        /// removes those columns. --json: the JSON object omits the dropped
        /// fields (including id, if excluded).
        #[arg(long)]
        exclude_fields: Option<String>,
        /// Override the server to run against
        #[arg(long)]
        server: Option<String>,
        /// Override the saved `creation_time` filter for this run.
        ///
        /// Same accepted forms as `bzr bug list --created-since`.
        #[arg(long, value_name = "DATE")]
        created_since: Option<String>,
        /// Override the saved `last_change_time` filter for this run.
        ///
        /// Same accepted forms as `bzr bug list --changed-since`.
        #[arg(long, value_name = "DATE")]
        changed_since: Option<String>,
        /// Override the saved Whiteboard filter for this run.
        #[arg(long)]
        whiteboard: Vec<String>,
        /// Override the saved Target Milestone filter.
        #[arg(long)]
        target_milestone: Vec<String>,
        /// Override the saved Version filter.
        #[arg(long)]
        version: Vec<String>,
        /// Override the saved Operating System filter.
        #[arg(long)]
        op_sys: Vec<String>,
        /// Override the saved Platform filter.
        #[arg(long)]
        platform: Vec<String>,
        /// Override the saved Resolution filter.
        #[arg(long)]
        resolution: Vec<String>,
        /// Override the saved QA Contact filter.
        #[arg(long)]
        qa_contact: Vec<String>,
        /// Override the saved URL filter.
        #[arg(long)]
        url: Vec<String>,
    },
}
