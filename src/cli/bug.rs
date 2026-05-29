use clap::{Args, Subcommand};

/// Shared `--fields` / `--exclude-fields` selection, flattened into the bug
/// query subcommands (`list`, `view`, `search`, `my`) so the pair is defined
/// once instead of repeated per variant.
#[derive(Args, Debug, Clone)]
pub struct FieldArgs {
    /// Fields to request from the server (comma-separated). In table output,
    /// selects which columns (or detail rows) to show, in order; under --json,
    /// the object contains only the selected fields (id only if requested).
    #[arg(long)]
    pub fields: Option<String>,
    /// Fields to drop from the server request (comma-separated). In table
    /// output, removes those columns/rows; under --json, the object omits the
    /// dropped fields (including id, if excluded).
    #[arg(long)]
    pub exclude_fields: Option<String>,
}

#[derive(Subcommand)]
#[expect(
    clippy::doc_markdown,
    reason = "doc examples are literal shell commands; wrapping URLs in <> or identifiers in backticks would degrade copy-paste UX"
)]
pub enum BugAction {
    /// List bugs that match the given filters.
    ///
    /// Filter flags (`--product`, `--component`, `--status`,
    /// `--assignee`, `--creator`, `--priority`, `--severity`) are
    /// repeatable for OR semantics within a category and AND across
    /// categories. Prefix any filter value with `!` to invert it
    /// (e.g. `--status '!CLOSED'`).
    ///
    /// `--summary` matches a substring against the bug's Summary field
    /// across all bug states (open and closed). It is the structured
    /// counterpart to `bzr bug search`, which uses Bugzilla's
    /// quicksearch syntax and defaults to open bugs only.
    ///
    /// `--limit` defaults to 50; raise it for broader scans, but very
    /// large values may exceed the server's max-results setting and
    /// return a truncated list. `--fields` and `--exclude-fields` control
    /// which fields are requested from the server; in table output they
    /// select and remove columns (in the given order). Under `--json` the
    /// output object is trimmed to the selected fields (gh-style):
    /// `--fields summary` returns `{"summary": ...}` with no `id` unless you
    /// ask for it, and `--exclude-fields id` drops `id`. With no selection the
    /// full object is returned. Unknown fields (typos, custom `cf_*` fields
    /// that have no table column) are skipped with a warning.
    ///
    /// `--created-since` / `--changed-since` filter by Bugzilla's
    /// `creation_time` / `last_change_time` fields. Both accept ISO
    /// 8601 (`YYYY-MM-DDTHH:MM:SSZ`) or a bare `YYYY-MM-DD` (treated
    /// as 00:00:00 UTC). Malformed input exits 7 before any network
    /// call.
    ///
    /// Eight additional field filters from bzl-parity issue #158:
    /// `--whiteboard`, `--target-milestone`, `--version`, `--op-sys`,
    /// `--platform`, `--resolution`, `--qa-contact`, `--url`. All
    /// repeatable for OR within a field; AND across fields. Prefix
    /// with `!` to invert. `--whiteboard` and `--url` are substring
    /// matches (negation uses `notsubstring`); the other six are
    /// exact match (negation uses `notequals`).
    ///
    /// Examples:
    ///
    ///   bzr bug list --product Firefox --status NEW --limit 25
    ///   bzr bug list --assignee me@example.com --status '!CLOSED'
    ///   bzr bug list --summary "kernel panic" --product Kernel
    ///   bzr bug list --id 100,101,102
    ///   bzr bug list --product Firefox --changed-since 2026-04-01
    ///
    /// See bzr-bug-search(1) for free-text search, bzr-bug-my(1) for
    /// caller-relative views, and bzr-query(1) for saving a filter
    /// combination by name.
    #[command(verbatim_doc_comment)]
    List {
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
        /// Filter by bug IDs
        #[arg(long)]
        id: Vec<u64>,
        /// Filter by alias
        #[arg(long)]
        alias: Option<String>,
        /// Substring match on the Summary field (matches across all bug states)
        #[arg(long)]
        summary: Option<String>,
        /// Max number of results
        #[arg(long, default_value = "50")]
        limit: u32,
        #[command(flatten)]
        field_args: FieldArgs,
        /// Filter to bugs created at or after this date.
        ///
        /// Accepts `YYYY-MM-DD` (interpreted as 00:00:00 UTC),
        /// `YYYY-MM-DDTHH:MM:SS`, `YYYY-MM-DDTHH:MM:SSZ`, or
        /// `YYYY-MM-DDTHH:MM:SS±HH:MM`. Malformed input exits 7.
        #[arg(long, value_name = "DATE")]
        created_since: Option<String>,
        /// Filter to bugs last modified at or after this date.
        ///
        /// Same accepted forms as `--created-since`.
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
    /// View one or more bugs by ID or alias.
    ///
    /// Single-ID: prints the bug's full record (summary, status,
    /// assignee, priority, CC list, depends-on, blocks, and the most
    /// recent comments) as a formatted detail block or JSON object.
    ///
    /// Multi-ID: emits one detail block per bug separated by a
    /// horizontal divider, in argument order. Aliases and numeric IDs
    /// may be mixed. JSON output for multi-ID becomes a wrapped
    /// `{"bugs": [...], "failed": [...]}` object — `failed` is always
    /// present (empty array when no failures) so `jq` consumers can
    /// rely on `.bugs[]`. Single-ID JSON output is unchanged
    /// (a bare `Bug` object).
    ///
    /// `--permissive` (multi-ID only) suppresses per-bug access
    /// failures: inaccessible bugs are surfaced as inline
    /// `Bug #N — UNAVAILABLE` placeholder blocks instead of bailing
    /// the whole call. Exit code is 0 even when some bugs fail.
    /// Session-wide failures (transport, auth, security) still bail.
    ///
    /// Use `--fields` to fetch only specific fields (faster on large
    /// bugs over REST); `--exclude-fields` is the inverse. Under `--json`
    /// the returned object is trimmed to the selected fields (gh-style) on
    /// every transport, since trimming happens client-side after the fetch.
    /// On XML-RPC servers the full bug is fetched regardless of the field
    /// list, so there the selection only controls which detail rows (table)
    /// or object keys (JSON) appear, not what is sent over the wire.
    ///
    /// Under `--json`, `bug view` stays lenient when the selection resolves
    /// to nothing known: an unknown or mistyped `--fields`, or an
    /// `--exclude-fields` covering every field, yields an empty `{}` object
    /// and exits 0, with a one-line warning on stderr. `bug list`, `my`,
    /// `search`, and `query run` instead exit 7 in that case. So a `{}`
    /// object plus a zero exit can mean a field name was misspelled — check
    /// stderr.
    ///
    /// Examples:
    ///
    ///   bzr bug view 12345
    ///   bzr bug view 12345 12346 12347
    ///   bzr bug view 12345 my-alias 12347 --permissive
    ///   bzr bug view 12345 --json | jq .summary
    ///   bzr bug view my-alias --fields id,summary,status
    ///
    /// See bzr-bug-history(1) for the change log and
    /// bzr-comment-list(1) for the full comment thread.
    #[command(verbatim_doc_comment)]
    View {
        /// Bug ID(s) or alias(es). Aliases and numeric IDs may be mixed.
        #[arg(required = true, num_args = 1..)]
        ids: Vec<String>,
        /// Continue past per-bug failures (multi-ID only).
        ///
        /// When set, inaccessible bugs (NotFound or Bug.get fault
        /// codes 100/101/102) are reported as inline placeholder
        /// rows and the command exits 0. Without `--permissive`,
        /// the first per-bug failure aborts the whole call. Has
        /// no effect on session-wide errors (transport, auth,
        /// security) — those always bail.
        #[arg(long)]
        permissive: bool,
        #[command(flatten)]
        field_args: FieldArgs,
    },
    /// Search bugs using Bugzilla quicksearch or by parsing a Bugzilla URL.
    ///
    /// The positional `query` is passed verbatim to the server's
    /// quicksearch engine, which searches summary, description, and
    /// comments and understands operators (`@user` for assignee,
    /// `:product` for product, etc.). Mutually exclusive with
    /// `--from-url`, which parses a Bugzilla `buglist.cgi` URL and
    /// reproduces the same filter set against the configured server.
    /// Unrecognized URL parameters are passed through verbatim.
    ///
    /// Important: quicksearch defaults to OPEN bugs only. To include
    /// closed/resolved bugs, prepend the bare token `ALL` to the
    /// query (e.g. `ALL kernel panic`). For a Summary-field-only
    /// substring match across all bug states with no quicksearch
    /// tokenization or status defaults at play, use
    /// `bzr bug list --summary <text>`.
    ///
    /// `--save-as` (only valid with `--from-url`) saves the parsed
    /// query for reuse; if no name is given it defaults to the URL's
    /// `known_name` parameter when present, otherwise an
    /// auto-generated name. Saved queries are managed with
    /// `bzr query`.
    ///
    /// Examples:
    ///
    ///   bzr bug search "kernel panic" --limit 10
    ///   bzr bug search "ALL kernel panic" --limit 10      # all states, summary+description+comments
    ///   bzr bug list --summary "kernel panic" --limit 10  # all states, summary only
    ///   bzr bug search --from-url 'https://bz/buglist.cgi?product=Firefox'
    ///   bzr bug search --from-url '...' --save-as firefox-bugs
    ///
    /// See bzr-bug-list(1) for filter-flag based listing and
    /// bzr-query(1) for managing saved queries directly.
    #[command(verbatim_doc_comment)]
    Search {
        /// Quicksearch query (mutually exclusive with `--from-url`).
        ///
        /// Passed to the server's quicksearch engine, which searches
        /// summary, description, and comments and DEFAULTS TO OPEN
        /// BUGS ONLY. Prepend the bare token `ALL` to include closed
        /// bugs (`ALL kernel panic`); for a Summary-field-only match
        /// across all states, use `bzr bug list --summary <text>`.
        #[arg(conflicts_with = "from_url")]
        query: Option<String>,
        /// Execute a search from a Bugzilla `buglist.cgi` URL.
        ///
        /// Parses the URL's query parameters into known filters
        /// where possible; unrecognized parameters are passed
        /// through to the API verbatim. Pair with `--save-as` to
        /// persist the parsed query as a named entry usable by
        /// `bzr query run`.
        #[arg(long)]
        from_url: Option<String>,
        /// Save the parsed `--from-url` query for future reuse.
        ///
        /// Only valid with `--from-url`. If a name is provided,
        /// the query is stored under that name. If `--save-as` is
        /// given without a value, the URL's `known_name` query
        /// parameter is used as the name; if neither is present,
        /// the command fails with input-validation (exit code 7).
        /// Saved queries are managed via `bzr query`.
        #[arg(long, requires = "from_url", num_args = 0..=1, default_missing_value = "")]
        save_as: Option<String>,
        /// Max number of results (default: 50)
        #[arg(long)]
        limit: Option<u32>,
        #[command(flatten)]
        field_args: FieldArgs,
    },
    /// Show the change history for a single bug.
    ///
    /// Prints every recorded change to the bug's fields (status,
    /// assignee, comments added, attachments, etc.) in chronological
    /// order, including the user who made each change. Use `--since`
    /// (ISO 8601 date or datetime) to limit the output to changes
    /// after a given point.
    ///
    /// Examples:
    ///
    ///   bzr bug history 12345
    ///   bzr bug history 12345 --since 2026-01-01
    ///   bzr bug history 12345 --since 2026-04-15T00:00:00Z --json
    ///
    /// See bzr-bug-view(1) for the current state of a bug.
    #[command(verbatim_doc_comment)]
    History {
        /// Bug ID
        id: u64,
        /// Only show changes after this date (ISO 8601)
        #[arg(long)]
        since: Option<String>,
    },
    /// Create a new bug under a product and component.
    ///
    /// `--product` and `--component` are required unless a saved
    /// template (`--template`) supplies them; CLI flags override
    /// template values. Some Bugzilla installations also require
    /// `--op-sys` and `--rep-platform` -- the API call fails with
    /// exit code 4 (Api) when the server demands a field that
    /// wasn't provided.
    ///
    /// Description sources, highest priority first:
    ///
    ///   1. `--description "text"` (literal)
    ///   2. `--description-file PATH` (UTF-8 file contents)
    ///   3. piped stdin (when stdin is not a TTY)
    ///   4. `$EDITOR` (when stdin is a TTY and none of the above)
    ///
    /// `--description` and `--description-file` are mutually
    /// exclusive. When the editor flow is active, `--summary` is
    /// optional: the first non-empty line of the buffer becomes
    /// the summary and the rest becomes the description. A
    /// `git commit -v`-style sentinel divider separates editable
    /// content from informational field reminders.
    ///
    /// On success, prints the new bug ID, alias (if assigned), and
    /// URL to stdout. With `--json`, the same fields are emitted as
    /// a JSON object suitable for piping into scripts.
    ///
    /// Examples:
    ///
    ///   bzr bug create --product Fedora --component kernel \
    ///     --summary "Boot failure on 6.x" \
    ///     --description "System hangs at initramfs"
    ///   bzr bug create --product Fedora --component kernel \
    ///     --description-file /tmp/desc.txt --summary "Boot failure"
    ///   bzr bug create --product Fedora --component kernel
    ///     # opens $EDITOR; first non-empty line of the buffer
    ///     # becomes the summary
    ///   bzr bug create --template security-bug --summary "XSS in login"
    ///
    /// Exit codes: 0 on success, 4 on Bugzilla API error, 7 on
    /// input validation (missing --summary outside the editor flow,
    /// empty editor buffer, missing or non-UTF-8 --description-file,
    /// $EDITOR exited non-zero), 9 on auth failure.
    ///
    /// See bzr-bug-clone(1) for cloning an existing bug,
    /// bzr-template(1) for managing templates, and bzr-field(1) for
    /// discovering valid `--priority`, `--severity`, and `--status`
    /// values.
    #[command(verbatim_doc_comment)]
    Create {
        /// Use a saved template for default field values.
        ///
        /// References a named template from `bzr template list`.
        /// When set, fields stored in the template (product,
        /// component, version, priority, severity, assignee,
        /// op-sys, rep-platform, description) are used as defaults
        /// for this `create` invocation; CLI flags override
        /// template values.
        #[arg(long)]
        template: Option<String>,
        /// Product name (required unless supplied by `--template`).
        ///
        /// Required unless the chosen template provides a product.
        /// When both are set, this CLI value wins.
        #[arg(long)]
        product: Option<String>,
        /// Component name (required unless supplied by `--template`).
        ///
        /// Required unless the chosen template provides a
        /// component. When both are set, this CLI value wins. The
        /// component must exist on the chosen product -- discover
        /// valid names via `bzr product view <product>`.
        #[arg(long)]
        component: Option<String>,
        /// Bug summary (required unless the editor flow is active)
        #[arg(long)]
        summary: Option<String>,
        /// Version
        #[arg(long)]
        version: Option<String>,
        /// Bug description
        #[arg(long, conflicts_with = "description_file")]
        description: Option<String>,
        /// Read the bug description from a UTF-8 file.
        ///
        /// Mutually exclusive with `--description`. The file path
        /// must exist and be readable; non-existent paths or
        /// non-UTF-8 contents fail with exit code 7.
        #[arg(long, value_name = "PATH", conflicts_with = "description")]
        description_file: Option<std::path::PathBuf>,
        /// Priority
        #[arg(long)]
        priority: Option<String>,
        /// Severity
        #[arg(long)]
        severity: Option<String>,
        /// Assignee
        #[arg(long)]
        assignee: Option<String>,
        /// Operating system (required by some Bugzilla installations)
        #[arg(long)]
        op_sys: Option<String>,
        /// Hardware platform (required by some Bugzilla installations)
        #[arg(long)]
        rep_platform: Option<String>,
        /// Bug IDs that this bug blocks (comma-separated)
        #[arg(long, value_delimiter = ',')]
        blocks: Vec<u64>,
        /// Bug IDs that this bug depends on (comma-separated)
        #[arg(long, value_delimiter = ',')]
        depends_on: Vec<u64>,
    },
    /// Show bugs related to the authenticated user.
    ///
    /// Default view: bugs assigned to the caller. `--created`
    /// switches to bugs the caller filed; `--cc` switches to bugs
    /// the caller is CC'd on; `--all` shows all three categories
    /// at once (assigned, created, CC'd) and conflicts with the
    /// other two flags.
    ///
    /// `--limit` is per category, not total -- with `--all` and
    /// `--limit 50`, up to 150 rows may be returned. `--status`
    /// filters across whichever category is active, with the same
    /// repeatability and `!`-prefix semantics as `bzr bug list`.
    ///
    /// Examples:
    ///
    ///   bzr bug my
    ///   bzr bug my --created --status NEW
    ///   bzr bug my --all --limit 25
    ///
    /// See bzr-bug-list(1) for filter-driven listing without the
    /// caller-relative shortcuts and bzr-whoami(1) to confirm which
    /// account `my` resolves to.
    #[command(verbatim_doc_comment)]
    My {
        /// Show bugs I created (instead of assigned to me).
        ///
        /// Mutually exclusive with `--all`.
        #[arg(long)]
        created: bool,
        /// Show bugs I'm CC'd on (instead of assigned to me).
        ///
        /// Mutually exclusive with `--all`.
        #[arg(long)]
        cc: bool,
        /// Show all bugs related to me (assigned + created + CC'd).
        ///
        /// Mutually exclusive with `--created` and `--cc`. Output
        /// is grouped into the three categories; `--limit` applies
        /// per category, so the total can be up to 3x the limit.
        #[arg(long, conflicts_with_all = ["created", "cc"])]
        all: bool,
        /// Filter by status (repeatable for OR; prefix with ! to exclude)
        #[arg(long)]
        status: Vec<String>,
        /// Max results per category (assigned/created/cc).
        ///
        /// With `--all`, the limit applies independently to each of
        /// the three categories, so up to 3x this value may be
        /// returned. With `--created`, `--cc`, or no view flag,
        /// the limit applies to the single active category.
        #[arg(long, default_value = "50")]
        limit: u32,
        #[command(flatten)]
        field_args: FieldArgs,
    },
    /// Clone an existing bug, optionally overriding fields.
    ///
    /// Copies the source bug's product, component, version, summary,
    /// description, priority, severity, assignee, op-sys,
    /// rep-platform, CC list, and keywords into a new bug. Pass any
    /// of the override flags (`--summary`, `--product`, ...) to
    /// change values for the clone; unspecified fields inherit from
    /// the source.
    ///
    /// By default the new bug gets a "Cloned from bug #N" comment;
    /// disable with `--no-comment`. Use `--add-depends-on` to link
    /// the new bug as a dependency of the source, or `--add-blocks`
    /// to make it block the source. `--no-cc` and `--no-keywords`
    /// skip copying those lists.
    ///
    /// Examples:
    ///
    ///   bzr bug clone 12345
    ///   bzr bug clone 12345 --summary "Backport to RHEL 9" \
    ///     --version "RHEL 9.4" --add-depends-on
    ///
    /// See bzr-bug-create(1) for filing a brand-new bug.
    #[command(verbatim_doc_comment)]
    Clone {
        /// Source bug ID or alias
        id: String,
        /// Override summary
        #[arg(long)]
        summary: Option<String>,
        /// Override product
        #[arg(long)]
        product: Option<String>,
        /// Override component
        #[arg(long)]
        component: Option<String>,
        /// Override version
        #[arg(long)]
        version: Option<String>,
        /// Override description
        #[arg(long)]
        description: Option<String>,
        /// Override priority
        #[arg(long)]
        priority: Option<String>,
        /// Override severity
        #[arg(long)]
        severity: Option<String>,
        /// Override assignee
        #[arg(long)]
        assignee: Option<String>,
        /// Override operating system
        #[arg(long)]
        op_sys: Option<String>,
        /// Override hardware platform
        #[arg(long)]
        rep_platform: Option<String>,
        /// Skip adding "Cloned from bug #N" comment
        #[arg(long)]
        no_comment: bool,
        /// Make the new bug depend on the source bug
        #[arg(long)]
        add_depends_on: bool,
        /// Make the new bug block the source bug
        #[arg(long)]
        add_blocks: bool,
        /// Don't copy the CC list from the source bug
        #[arg(long)]
        no_cc: bool,
        /// Don't copy keywords from the source bug
        #[arg(long)]
        no_keywords: bool,
    },
    /// Update one or more bugs with the same set of changes.
    ///
    /// Accepts one or more bug IDs as positional args. All field
    /// changes (`--status`, `--resolution`, `--dupe-of`, `--assignee`,
    /// `--priority`, `--severity`, `--summary`, `--whiteboard`) are
    /// applied to every bug in the list. When closing a bug, both
    /// `--status` and `--resolution` typically need to be set
    /// together (e.g. `--status RESOLVED --resolution FIXED`).
    ///
    /// `--dupe-of <ID>` marks the bug as a duplicate. Bugzilla sets
    /// status/resolution automatically; do not set status/resolution
    /// just to make the duplicate transition.
    ///
    /// `--flag` accepts Bugzilla flag syntax: `name?`, `name+`,
    /// `name-`, `name?(user@example.com)`, or `name?,!` to clear.
    /// Repeatable.
    ///
    /// `--comment <BODY>` (or `--comment-file <PATH>`) posts a comment
    /// atomically with the field changes — a single `Bug.update`
    /// round-trip rather than a separate `bzr comment add` call.
    /// `--comment-private` marks it private. Empty / whitespace-only
    /// bodies are rejected (exit 7).
    ///
    /// List-typed fields support `*-add` / `*-remove` pairs for
    /// incremental edits: `--blocks`, `--depends-on`, `--keywords`,
    /// `--cc`, `--groups`, and `--see-also`. The first five accept
    /// comma-separated values. `--see-also-add` / `--see-also-remove`
    /// do not split on commas — repeat the flag to pass multiple URLs.
    /// Additional update fields include `--alias`, `--deadline`,
    /// `--estimated-time`, `--remaining-time`, `--work-time`,
    /// `--reset-assigned-to`, and `--reset-qa-contact`.
    ///
    /// On batch updates, partial failures (some bugs updated,
    /// others rejected) exit with code 11 (BatchPartialFailure) and
    /// the JSON output enumerates per-bug results.
    ///
    /// Examples:
    ///
    ///   bzr bug update 100 --status RESOLVED --resolution FIXED
    ///   bzr bug update 100 --dupe-of 200
    ///   bzr bug update 100 200 300 --priority high --flag review+
    ///   bzr bug update 100 --status RESOLVED --resolution FIXED \
    ///     --comment "Fixed by patch in #200"
    ///   bzr bug update 100 --blocks-add 200,201 \
    ///     --depends-on-remove 99
    ///   bzr bug update 100 --keywords-add fix-needed,regression \
    ///     --cc-add alice@example.com \
    ///     --see-also-add <https://example.com/issue/42>
    ///
    /// See bzr-bug-create(1) for new bugs, bzr-bug-clone(1) for
    /// cloning, and bzr-comment-add(1) for adding a comment as part
    /// of a status change.
    #[command(verbatim_doc_comment)]
    Update {
        /// Bug ID(s).
        ///
        /// One or more IDs. When more than one is supplied, the
        /// same field changes are applied to every bug; partial
        /// failures (some bugs updated, others rejected) exit with
        /// code 11 and the JSON output enumerates per-bug results.
        #[arg(required = true, num_args = 1..)]
        ids: Vec<u64>,
        /// New status (e.g. `NEW`, `ASSIGNED`, `RESOLVED`, `CLOSED`).
        ///
        /// When closing a bug, `--resolution` must usually be set
        /// in the same call. Discover valid values via
        /// `bzr field list status`.
        #[arg(long, conflicts_with = "dupe_of")]
        status: Option<String>,
        /// Resolution to set when closing a bug.
        ///
        /// Required by most workflows when `--status` transitions
        /// to a closed state (e.g. `RESOLVED`, `VERIFIED`).
        /// Discover valid values via `bzr field list resolution`.
        #[arg(long, conflicts_with = "dupe_of")]
        resolution: Option<String>,
        /// Mark this bug as a duplicate of another bug.
        ///
        /// Forwards Bugzilla's `dupe_of` field. Bugzilla handles the
        /// status/resolution transition to RESOLVED/DUPLICATE.
        #[arg(long, value_name = "ID")]
        dupe_of: Option<u64>,
        /// Set this bug's alias.
        ///
        /// Bugzilla only allows alias updates for a single bug at a time.
        #[arg(long, value_name = "ALIAS")]
        alias: Option<String>,
        /// Set the deadline date (`YYYY-MM-DD`).
        #[arg(long, value_name = "DATE")]
        deadline: Option<String>,
        /// Set the total estimated time in hours.
        #[arg(long, value_name = "HOURS")]
        estimated_time: Option<f64>,
        /// Set the remaining time in hours.
        #[arg(long, value_name = "HOURS")]
        remaining_time: Option<f64>,
        /// Add work time in hours for this update.
        #[arg(long, value_name = "HOURS")]
        work_time: Option<f64>,
        /// Reset assignee to the component default.
        #[arg(long)]
        reset_assigned_to: bool,
        /// Reset QA contact to the component default.
        #[arg(long)]
        reset_qa_contact: bool,
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
        /// Post a comment atomically with the field changes.
        ///
        /// Mutually exclusive with `--comment-file`. Use
        /// `--comment-private` to mark the comment private. Empty /
        /// whitespace-only bodies are rejected (exit 7).
        #[arg(long, value_name = "BODY", conflicts_with = "comment_file")]
        comment: Option<String>,
        /// Read the comment body from a UTF-8 file.
        ///
        /// Mutually exclusive with `--comment`. The file must exist
        /// and be readable; non-existent paths or non-UTF-8 contents
        /// fail with exit code 7. Empty / whitespace-only contents
        /// are also rejected.
        #[arg(long, value_name = "PATH", conflicts_with = "comment")]
        comment_file: Option<std::path::PathBuf>,
        /// Mark the comment private (visible only to users with
        /// elevated permissions on the server).
        ///
        /// Requires `--comment` or `--comment-file`; using
        /// `--comment-private` alone is a usage error (exit 7).
        #[arg(long)]
        comment_private: bool,
        /// Set, request, or clear a flag using Bugzilla flag syntax.
        ///
        /// Repeatable. Accepted forms:
        /// `name+` (granted), `name-` (denied), `name?` (request),
        /// `name?(user@example.com)` (request a specific user), or
        /// `name?,!` to clear an existing flag.
        #[arg(long)]
        flag: Vec<String>,
        /// Add bug IDs to the blocks list (comma-separated).
        ///
        /// Combine with `--blocks-remove` for incremental edits.
        /// To replace the list entirely, the bug must be edited
        /// through the Bugzilla web UI.
        #[arg(long, value_delimiter = ',')]
        blocks_add: Vec<u64>,
        /// Remove bug IDs from the blocks list (comma-separated).
        #[arg(long, value_delimiter = ',')]
        blocks_remove: Vec<u64>,
        /// Add bug IDs to the depends-on list (comma-separated).
        ///
        /// Combine with `--depends-on-remove` for incremental
        /// edits.
        #[arg(long, value_delimiter = ',')]
        depends_on_add: Vec<u64>,
        /// Remove bug IDs from the depends-on list (comma-separated).
        #[arg(long, value_delimiter = ',')]
        depends_on_remove: Vec<u64>,
        /// Add keywords (comma-separated).
        ///
        /// Combine with `--keywords-remove` for incremental edits.
        #[arg(long, value_delimiter = ',')]
        keywords_add: Vec<String>,
        /// Remove keywords (comma-separated).
        #[arg(long, value_delimiter = ',')]
        keywords_remove: Vec<String>,
        /// Add CC entries (comma-separated).
        ///
        /// Accepts usernames or email addresses; format is
        /// server-defined.
        #[arg(long, value_delimiter = ',')]
        cc_add: Vec<String>,
        /// Remove CC entries (comma-separated).
        #[arg(long, value_delimiter = ',')]
        cc_remove: Vec<String>,
        /// Add groups (comma-separated).
        ///
        /// Group operations require permission; failures surface
        /// from the server.
        #[arg(long, value_delimiter = ',')]
        groups_add: Vec<String>,
        /// Remove groups (comma-separated).
        #[arg(long, value_delimiter = ',')]
        groups_remove: Vec<String>,
        /// Add a see-also URL.
        ///
        /// Repeat the flag to add multiple URLs (URLs may contain
        /// commas, so no comma-list parsing is performed).
        #[arg(long)]
        see_also_add: Vec<String>,
        /// Remove a see-also URL.
        ///
        /// Repeat the flag to remove multiple URLs.
        #[arg(long)]
        see_also_remove: Vec<String>,
    },
}
