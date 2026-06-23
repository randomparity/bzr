use clap::Args;

use crate::cli::bug::{FieldArgs, PageArgs, SortArgs};

pub const LONG_ABOUT: &str = r#"Search bugs using Bugzilla quicksearch or by parsing a Bugzilla URL.

The positional `query` is passed verbatim to the server's
quicksearch engine, which searches summary, description, and
comments and understands operators (`@user` for assignee,
`:product` for product, etc.). Mutually exclusive with
`--from-url`, which parses a Bugzilla `buglist.cgi` URL and
reproduces the same filter set against the configured server.
Unrecognized URL parameters are passed through verbatim.

Important: quicksearch defaults to OPEN bugs only. To include
closed/resolved bugs, prepend the bare token `ALL` to the
query (e.g. `ALL kernel panic`). For a Summary-field-only
substring match across all bug states with no quicksearch
tokenization or status defaults at play, use
`bzr bug list --summary <text>`.

`--save-as` (only valid with `--from-url`) saves the parsed
query for reuse; if no name is given it defaults to the URL's
`known_name` parameter when present, otherwise an
auto-generated name. Saved queries are managed with
`bzr query`.

Examples:

  bzr bug search "kernel panic" --limit 10
  bzr bug search "ALL kernel panic" --limit 10      # all states, summary+description+comments
  bzr bug list --summary "kernel panic" --limit 10  # all states, summary only
  bzr bug search --from-url 'https://bz/buglist.cgi?product=Firefox'
  bzr bug search --from-url '...' --save-as firefox-bugs

See bzr-bug-list(1) for filter-flag based listing and
bzr-query(1) for managing saved queries directly."#;

/// Arguments for `bug search`.
#[derive(Args, Debug)]
pub struct SearchArgs {
    /// Quicksearch query (mutually exclusive with `--from-url`).
    ///
    /// Passed to the server's quicksearch engine, which searches
    /// summary, description, and comments and DEFAULTS TO OPEN
    /// BUGS ONLY. Prepend the bare token `ALL` to include closed
    /// bugs (`ALL kernel panic`); for a Summary-field-only match
    /// across all states, use `bzr bug list --summary <text>`.
    #[arg(conflicts_with = "from_url")]
    pub query: Option<String>,
    /// Execute a search from a Bugzilla `buglist.cgi` URL.
    ///
    /// Parses the URL's query parameters into known filters
    /// where possible; unrecognized parameters are passed
    /// through to the API verbatim. Pair with `--save-as` to
    /// persist the parsed query as a named entry usable by
    /// `bzr query run`.
    #[arg(long)]
    pub from_url: Option<String>,
    /// Save the parsed `--from-url` query for future reuse.
    ///
    /// Only valid with `--from-url`. If a name is provided,
    /// the query is stored under that name. If `--save-as` is
    /// given without a value, the URL's `known_name` query
    /// parameter is used as the name; if neither is present,
    /// the command fails with input-validation (exit code 7).
    /// Saved queries are managed via `bzr query`.
    #[arg(long, requires = "from_url", num_args = 0..=1, default_missing_value = "")]
    pub save_as: Option<String>,
    /// Max number of results (default: 50)
    #[arg(long)]
    pub limit: Option<u32>,
    /// Print only the number of matching bugs, not the rows.
    ///
    /// Counts all matches (bounded by the server's max-results setting)
    /// and prints just the integer (table) or `{"count": N}` (JSON).
    #[arg(long)]
    pub count: bool,
    #[command(flatten)]
    pub field_args: FieldArgs,
    #[command(flatten)]
    pub sort_args: SortArgs,
    #[command(flatten)]
    pub page_args: PageArgs,
}
