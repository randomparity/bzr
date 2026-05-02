use clap::Subcommand;

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
    /// Examples:
    ///
    ///   bzr query save firefox-new --product Firefox --status NEW
    ///   bzr query save crashes --search "crash in tab"
    ///   bzr query save my-saved \
    ///     --from-url 'https://bz/buglist.cgi?product=Firefox'
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
        #[arg(long, conflicts_with_all = ["search", "product", "component", "status", "assignee", "creator", "priority", "severity"])]
        from_url: Option<String>,
        /// Free-text search query (creates a `search`-kind saved query).
        ///
        /// Mutually exclusive with `--from-url` and the structured
        /// filter flags. Stores the query as a free-text search;
        /// `bzr query run <name>` then issues the same search
        /// against the configured server.
        #[arg(long)]
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
    /// Examples:
    ///
    ///   bzr query run firefox-new
    ///   bzr query run firefox-new --limit 10
    ///   bzr query run firefox-new --server staging
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
        /// Override the saved fields selection
        #[arg(long)]
        fields: Option<String>,
        /// Override the saved exclude-fields selection
        #[arg(long)]
        exclude_fields: Option<String>,
        /// Override the server to run against
        #[arg(long)]
        server: Option<String>,
    },
}
